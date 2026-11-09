//! Combat damage: melee swing hits, shared damage application and npc death
//! handling. The ranged counterpart lives in [`super::ranged`].

use bevy::prelude::*;

use super::selection::{Selection, SelectionKind};
use crate::gameplay::distance::ClassifyDistance;
use crate::gameplay::objects::{NpcModel, PlayerCharacter};
use crate::graphics::AnimationController;
use crate::graphics::{NpcHitReaction, play_hit_reaction};
use crate::gui::GameLog;
use crate::zengin::instance_index::InstanceIndex;
use crate::zengin::loaders::vdf_reader::ZenginVfs;
use crate::zengin::loaders::vfs::item_model_path;
use crate::zengin::npc_routines::{AnimationFiles, NpcEquippedWeapon, NpcOverlay, NpcWeaponMode};
use crate::zengin::resources::{CachedAssetHandles, ZenGinModelComponent};
use crate::zengin::script::TransferItems;
use crate::zengin::script::component::ScriptResource;
use crate::zengin::script::state::WeaponMode;
use crate::zengin::script::symbol_id::InstanceId;
use crate::zengin::{GameItem, GameNpc};

/// Query data of the npc state needed to apply hits and deaths.
/// Shared with the ranged hit system in [`super::ranged`].
pub(super) type NpcHitQueryData = (
    &'static GameNpc,
    &'static GlobalTransform,
    &'static NpcModel,
    Option<&'static NpcOverlay>,
    Option<&'static mut AnimationController>,
    Option<&'static NpcHitReaction>,
);

/// An npc died (hitpoints reached 0): its inventory moves to the player, the
/// drawn weapon drops on the ground and the npc entity is despawned.
#[derive(Message)]
pub struct NpcDied {
    pub entity: Entity,
    pub instance_id: InstanceId,
    /// Position where the dropped weapon is placed.
    pub position: Vec3,
}

/// Base damage of an unarmed melee swing. There is no script constant for it,
/// the engine hardcodes the fist damage.
const FIST_DAMAGE: i32 = 5;

/// Horizontal distance within which an npc swing can connect. Slightly larger
/// than the attack range so a target stepping back mid-swing is still hit.
const MELEE_HIT_RANGE: f32 = 2.5;

/// Melee swing impact: written by the animation sequencer when the attack
/// swing animation of an entity crosses its optimal frame (see
/// `AutoAnimSequencer`), the counterpart of the ranged `ArrowShot`.
#[derive(Message, Debug)]
pub struct MeleeHit {
    /// Animation model entity of the attacker, player or npc.
    pub shooter: Entity,
}

/// Applies the melee swing damage to the currently selected npc when a
/// [`MeleeHit`] arrives. The damage is the drawn weapon's `damagetotal`; a
/// swing without a selected npc hits nothing. The npc dies when its hitpoints
/// reach 0.
pub(super) fn melee_swing_hit(
    mut commands: Commands,
    mut hits: MessageReader<MeleeHit>,
    selection: Res<Selection>,
    mut npcs: Query<NpcHitQueryData, Without<PlayerCharacter>>,
    player_npc: Query<&GameNpc, With<PlayerCharacter>>,
    player_weapon: Query<&NpcWeaponMode, With<PlayerCharacter>>,
    gothic_script: Res<ScriptResource>,
    files: Res<AnimationFiles>,
    mut handles: ResMut<CachedAssetHandles>,
    asset_server: Res<AssetServer>,
    mut game_log: ResMut<GameLog>,
    mut npc_died: MessageWriter<NpcDied>,
) {
    let weapon_mode = player_weapon
        .single()
        .map_or(WeaponMode::Fists, |mode| mode.0);
    for hit in hits.read() {
        // Npc swings are handled by `npc_melee_swing_hit`.
        if !player_npc.contains(hit.shooter) {
            continue;
        }
        if selection.kind != Some(SelectionKind::Npc) {
            continue;
        }
        let Some(target) = selection.target else {
            continue;
        };
        let Ok((npc, global_tr, npc_model, overlay, controller, reaction)) = npcs.get_mut(target)
        else {
            continue;
        };
        let npc_data = NpcDamageData {
            npc,
            global_tr,
            model: npc_model,
            overlay,
            controller,
            reaction,
        };
        let Ok(player_npc) = player_npc.single() else {
            continue;
        };
        let player_id = player_npc.instance_id();

        let damage = {
            let mut script = gothic_script.lock();
            let weapon = script
                .npc_state(player_id)
                .and_then(|player| player.equipment.melee)
                .filter(|_| weapon_mode == WeaponMode::Melee);
            let Some(weapon) = weapon else {
                warn!(
                    "Melee hit ignored: player({}) swing ended without an equipped melee weapon",
                    player_id.value()
                );
                continue;
            };
            script.item_damage_total(weapon)
        };
        damage_npc(
            &mut commands,
            target,
            npc_data,
            damage,
            player_id,
            &gothic_script,
            &files,
            &mut handles,
            &asset_server,
            &mut game_log,
            &mut npc_died,
        );
    }
}

/// Applies a melee hit of an npc to its `Npc_SetTarget` target, player or npc.
/// The damage is the drawn melee weapon's `damagetotal`, or [`FIST_DAMAGE`] for
/// an unarmed npc. When a player's hitpoints reach 0 its inventory goes to the
/// killer and the player entity is despawned.
#[allow(clippy::too_many_arguments)]
pub(super) fn npc_melee_swing_hit(
    mut commands: Commands,
    mut hits: MessageReader<MeleeHit>,
    index: Res<InstanceIndex>,
    shooters: Query<(&GameNpc, &GlobalTransform), Without<PlayerCharacter>>,
    players: Query<(Entity, &GameNpc, &GlobalTransform, Option<&ChildOf>), With<PlayerCharacter>>,
    mut npcs: Query<NpcHitQueryData, Without<PlayerCharacter>>,
    transforms: Query<&GlobalTransform>,
    gothic_script: Res<ScriptResource>,
    files: Res<AnimationFiles>,
    mut handles: ResMut<CachedAssetHandles>,
    asset_server: Res<AssetServer>,
    mut game_log: ResMut<GameLog>,
    mut transfer_items: MessageWriter<TransferItems>,
    mut npc_died: MessageWriter<NpcDied>,
) {
    let mut player_dead = false;
    for hit in hits.read() {
        if player_dead {
            continue;
        }
        let Ok((npc, npc_tr)) = shooters.get(hit.shooter) else {
            // Player swings are handled by `melee_swing_hit`.
            continue;
        };
        let attacker_id = npc.instance_id();
        let Some(target_id) = gothic_script
            .lock()
            .npc_state(attacker_id)
            .and_then(|npc| npc.live.target)
        else {
            warn!(
                "Npc({}) melee hit ignored: no target set (Npc_SetTarget)",
                attacker_id.value()
            );
            continue;
        };
        let Some(&target_entity) = index.npcs(target_id).first() else {
            warn!(
                "Npc({}) melee hit ignored: target({target_id}) has no entity",
                attacker_id.value()
            );
            continue;
        };
        let Ok(target_tr) = transforms.get(target_entity) else {
            warn!(
                "Npc({}) melee hit ignored: target({target_id}) has no transform",
                attacker_id.value()
            );
            continue;
        };
        let distance = (target_tr.translation() - npc_tr.translation())
            .with_y(0.0)
            .length();
        if distance > MELEE_HIT_RANGE {
            warn!(
                "Npc({}) swing missed target({target_id}) at {distance:.2}m (> {MELEE_HIT_RANGE}m)",
                attacker_id.value()
            );
            continue;
        }

        let damage = {
            let mut script = gothic_script.lock();
            let melee = script
                .npc_state(attacker_id)
                .and_then(|npc| npc.equipment.melee);
            melee.map_or(FIST_DAMAGE, |item| script.item_damage_total(item))
        };

        if players.contains(target_entity) {
            let Ok((_, player_npc, _, parent)) = players.get(target_entity) else {
                continue;
            };
            let player_id = player_npc.instance_id();
            let Some(hitpoints) = gothic_script.lock().change_hitpoints(player_id, -damage) else {
                warn!(
                    "Npc melee hit ignored: no hitpoints data for player({})",
                    player_id.value()
                );
                continue;
            };
            info!(
                "Npc({}) dealt {damage} damage to player({player_id}), hitpoints now {hitpoints}",
                attacker_id.value()
            );
            game_log.add(format!("Took {damage} damage"));

            if hitpoints > 0 {
                continue;
            }
            player_dead = true;
            transfer_items.write(TransferItems {
                instance_from_id: player_id,
                instance_to_id: attacker_id,
            });
            // The player visual is a child of its physics body, the body is the
            // entity to remove (despawning it removes the visual with it).
            let entity_to_despawn = parent.map_or(target_entity, |parent| parent.parent());
            commands.entity(entity_to_despawn).try_despawn();
            info!(
                "Player({player_id}) died, killed by npc({})",
                attacker_id.value()
            );
            continue;
        }

        let Ok((target_npc, target_gt, model, overlay, controller, reaction)) =
            npcs.get_mut(target_entity)
        else {
            warn!(
                "Npc({}) melee hit ignored: target({target_id}) is not a damageable npc",
                attacker_id.value()
            );
            continue;
        };
        let npc_data = NpcDamageData {
            npc: target_npc,
            global_tr: target_gt,
            model,
            overlay,
            controller,
            reaction,
        };
        damage_npc(
            &mut commands,
            target_entity,
            npc_data,
            damage,
            attacker_id,
            &gothic_script,
            &files,
            &mut handles,
            &asset_server,
            &mut game_log,
            &mut npc_died,
        );
    }
}

/// Damage application shared by the melee swing and the arrow hit paths:
/// changes the npc hitpoints, logs, plays the `T_GOTHIT` hit reaction and
/// emits `NpcDied` when the hitpoints reach 0. Logs a warn when the npc has
/// no hitpoints data.
#[allow(clippy::too_many_arguments)]
pub(crate) fn damage_npc(
    commands: &mut Commands,
    target: Entity,
    npc_data: NpcDamageData,
    damage: i32,
    shooter_id: InstanceId,
    gothic_script: &Res<ScriptResource>,
    files: &AnimationFiles,
    handles: &mut CachedAssetHandles,
    asset_server: &AssetServer,
    game_log: &mut GameLog,
    npc_died: &mut MessageWriter<NpcDied>,
) {
    let npc_id = npc_data.npc.instance_id();
    let (hitpoints, shooter_is_player) = {
        let mut script = gothic_script.lock();
        let shooter_is_player = script.player_instance() == Some(shooter_id);
        let Some(hitpoints) = script.change_hitpoints(npc_id, -damage) else {
            warn!(
                "Damage ignored: no hitpoints data for npc({})",
                npc_id.value()
            );
            return;
        };
        (hitpoints, shooter_is_player)
    };
    if shooter_is_player {
        info!(
            "Player({}) dealt {damage} damage to npc({}), hitpoints now {hitpoints}",
            shooter_id.value(),
            npc_id.value()
        );
        game_log.add(format!("Dealt {damage} damage"));
    } else {
        info!(
            "Npc({}) dealt {damage} damage to npc({}), hitpoints now {hitpoints}",
            shooter_id.value(),
            npc_id.value()
        );
    }
    play_hit_reaction(
        commands,
        target,
        npc_data.model,
        npc_data.overlay,
        npc_data.controller,
        npc_data.reaction,
        files,
        handles,
        asset_server,
    );
    if hitpoints <= 0 {
        npc_died.write(NpcDied {
            entity: target,
            instance_id: npc_id,
            position: npc_data.global_tr.translation(),
        });
    }
}

/// The npc data [`damage_npc`] needs, gathered from the npc query by the
/// caller (the query item is destructured into it).
pub(crate) struct NpcDamageData<'a> {
    pub npc: &'a GameNpc,
    pub global_tr: &'a GlobalTransform,
    pub model: &'a NpcModel,
    pub overlay: Option<&'a NpcOverlay>,
    pub controller: Option<Mut<'a, AnimationController>>,
    pub reaction: Option<&'a NpcHitReaction>,
}

/// Removes the drawn weapon from the dead npc's inventory and spawns it on
/// the ground as a pickable item.
#[allow(clippy::too_many_arguments)]
fn drop_drawn_weapon(
    gothic_scripts: &ScriptResource,
    vfs: &ZenginVfs,
    handles: &mut CachedAssetHandles,
    asset_server: &Res<AssetServer>,
    commands: &mut Commands,
    instance_index: &mut InstanceIndex,
    npc_id: InstanceId,
    item: InstanceId,
    position: Vec3,
) {
    let mut script = gothic_scripts.lock();
    let removed = script.remove_inventory(npc_id, item, 1);
    if removed == 0 {
        warn!(
            "Dropped weapon item({}) of npc({}) was not in its inventory",
            item.value(),
            npc_id.value()
        );
    }
    let Some(handle) = script.item_handle(item) else {
        warn!("No item handle for dropped weapon item({})", item.value());
        return;
    };
    drop(script);
    instance_index.insert_item_handle(handle.clone());
    let Some(path) = item_model_path(gothic_scripts, vfs, item) else {
        warn!("No mesh path for dropped weapon item({item})");
        return;
    };
    let model_handle = handles.get_model_handle(asset_server, &path, None);
    commands.spawn((
        ClassifyDistance::default(),
        ZenGinModelComponent {
            model_handle,
            ..default()
        },
        Visibility::default(),
        Transform::from_translation(position),
        GameItem::new(handle),
    ));
}

/// Handles npc death: the inventory moves to the player, the drawn weapon
/// drops on the ground (removed from the inventory first so it is not
/// duplicated) and the npc entity is despawned.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn handle_npc_death(
    mut deaths: MessageReader<NpcDied>,
    mut selection: ResMut<Selection>,
    mut transfer_items: MessageWriter<TransferItems>,
    gothic_scripts: Res<ScriptResource>,
    vfs: Res<ZenginVfs>,
    mut handles: ResMut<CachedAssetHandles>,
    asset_server: Res<AssetServer>,
    mut instance_index: ResMut<InstanceIndex>,
    mut commands: Commands,
    npcs: Query<
        (
            Option<&GameNpc>,
            Option<&NpcEquippedWeapon>,
            Option<&NpcWeaponMode>,
        ),
        Without<PlayerCharacter>,
    >,
    player_npc: Query<&GameNpc, With<PlayerCharacter>>,
) {
    for death in deaths.read() {
        let Ok((npc, equipped, weapon_mode)) = npcs.get(death.entity) else {
            continue;
        };
        let npc_id = npc.map_or(death.instance_id, |npc| npc.instance_id());
        let position = death.position;
        // Only the drawn weapon drops on the ground; sheathed weapons stay
        // part of the inventory that moves to the player
        let dropped = match weapon_mode.map_or(WeaponMode::Fists, |mode| mode.0) {
            WeaponMode::Melee => equipped.and_then(|equipped| equipped.melee),
            WeaponMode::Ranged => equipped.and_then(|equipped| equipped.ranged),
            WeaponMode::Fists => None,
        };

        let Ok(player_npc) = player_npc.single() else {
            return;
        };
        let player_id = player_npc.instance_id();

        // Move the inventory to the player, it is logged by `handle_transfer_items`.
        transfer_items.write(TransferItems {
            instance_from_id: npc_id,
            instance_to_id: player_id,
        });

        // Drop the drawn weapon on the ground as a pickable item. Remove it
        // from the npc inventory first when present, otherwise the transfer
        // below would duplicate it.
        if let Some(item) = dropped {
            drop_drawn_weapon(
                &gothic_scripts,
                &vfs,
                &mut handles,
                &asset_server,
                &mut commands,
                &mut instance_index,
                npc_id,
                item,
                position,
            );
        }

        info!("Npc({npc_id}) died, moved its inventory to player({player_id})");
        commands.entity(death.entity).despawn();
        if selection.target == Some(death.entity) {
            selection.target = None;
            selection.kind = None;
            selection.name.clear();
        }
    }
}
