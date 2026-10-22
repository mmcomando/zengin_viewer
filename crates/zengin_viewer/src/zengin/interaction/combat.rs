//! Combat damage: melee swing hits, shared damage application and npc death
//! handling. The ranged counterpart lives in [`super::ranged`].

use bevy::prelude::*;

use super::selection::{Selection, SelectionKind};
use crate::gameplay::distance::ClassifyDistance;
use crate::gameplay::objects::{NpcModel, PlayerCharacter};
use crate::graphics::AnimationController;
use crate::graphics::{NpcHitReaction, play_hit_reaction};
use crate::gui::GameLog;
use crate::zengin::npc_routines::{AnimationFiles, NpcEquippedWeapon, NpcOverlay, NpcWeaponMode};
use crate::zengin::script::TransferItems;
use crate::zengin::script::component::ScriptResource;
use crate::zengin::script::state::WeaponMode;
use crate::zengin::script::symbol_id::InstanceId;
use crate::zengin::world::item_model_path;
use crate::zengin::zengin_resources::{MaterialHandles, ZenGinModelComponent};
use crate::zengin::{GameItem, GameNpc, ZenginVfs};

/// An npc died (hitpoints reached 0): its inventory moves to the player, the
/// drawn weapon drops on the ground and the npc entity is despawned.
#[derive(Message)]
pub struct NpcDied {
    pub entity: Entity,
    pub instance_id: InstanceId,
    /// Position where the dropped weapon is placed.
    pub position: Vec3,
}

/// Melee swing impact: written by the animation sequencer when the attack
/// swing animation of the player finishes (see `AutoAnimSequencer`), the
/// counterpart of the ranged `ArrowShot`.
#[derive(Message, Debug)]
pub struct MeleeHit {
    /// Animation model entity of the attacking player. Not used by the
    /// damage application (it goes through the player's script instance),
    /// kept for parity with [`crate::zengin::ranged::ArrowShot`].
    #[allow(dead_code)]
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
    mut npcs: Query<
        (
            &GameNpc,
            &GlobalTransform,
            &NpcModel,
            Option<&NpcOverlay>,
            Option<&mut AnimationController>,
            Option<&NpcHitReaction>,
        ),
        Without<PlayerCharacter>,
    >,
    player_npc: Query<&GameNpc, With<PlayerCharacter>>,
    player_weapon: Query<&NpcWeaponMode, With<PlayerCharacter>>,
    gothic_script: Res<ScriptResource>,
    files: Res<AnimationFiles>,
    mut handles: ResMut<MaterialHandles>,
    asset_server: Res<AssetServer>,
    mut game_log: ResMut<GameLog>,
    mut npc_died: MessageWriter<NpcDied>,
) {
    let weapon_mode = player_weapon
        .single()
        .map(|mode| mode.0)
        .unwrap_or(WeaponMode::Fists);
    for _hit in hits.read() {
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
        let player_id = player_npc.npc_instance_id;

        let damage = {
            let mut script = gothic_script.lock();
            let weapon = script
                .npc_instances()
                .get(&player_id)
                .and_then(|player| player.equipped_melee)
                .filter(|_| weapon_mode == WeaponMode::Melee);
            let Some(weapon) = weapon else {
                warn!(
                    "Melee hit ignored: player({}) swing ended without an equipped melee weapon",
                    player_id.value()
                );
                continue;
            };
            let Some(damage) = script.item_damage_total(weapon) else {
                warn!(
                    "Melee hit ignored: no damage data for weapon item({})",
                    weapon.value()
                );
                continue;
            };
            damage
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
    handles: &mut MaterialHandles,
    asset_server: &AssetServer,
    game_log: &mut GameLog,
    npc_died: &mut MessageWriter<NpcDied>,
) {
    let npc_id = npc_data.npc.npc_instance_id;
    let Some(hitpoints) = gothic_script.lock().change_hitpoints(npc_id, -damage) else {
        warn!(
            "Damage ignored: no hitpoints data for npc({})",
            npc_id.value()
        );
        return;
    };
    info!(
        "Player({}) dealt {damage} damage to npc({}), hitpoints now {hitpoints}",
        shooter_id.value(),
        npc_id.value()
    );
    game_log.add(format!("Dealt {damage} damage"));
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

/// Handles npc death: the inventory moves to the player, the drawn weapon
/// drops on the ground (removed from the inventory first so it is not
/// duplicated) and the npc entity is despawned.
#[allow(clippy::too_many_arguments)]
pub(super) fn handle_npc_death(
    mut deaths: MessageReader<NpcDied>,
    mut selection: ResMut<Selection>,
    mut transfer_items: MessageWriter<TransferItems>,
    gothic_scripts: Res<ScriptResource>,
    vfs: Res<ZenginVfs>,
    mut handles: ResMut<MaterialHandles>,
    asset_server: Res<AssetServer>,
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
        let npc_id = npc
            .map(|npc| npc.npc_instance_id)
            .unwrap_or(death.instance_id);
        let position = death.position;
        // Only the drawn weapon drops on the ground; sheathed weapons stay
        // part of the inventory that moves to the player
        let dropped = match weapon_mode.map(|mode| mode.0).unwrap_or(WeaponMode::Fists) {
            WeaponMode::Melee => equipped.and_then(|equipped| equipped.melee),
            WeaponMode::Ranged => equipped.and_then(|equipped| equipped.ranged),
            WeaponMode::Fists => None,
        };

        let Ok(player_npc) = player_npc.single() else {
            return;
        };
        let player_id = player_npc.npc_instance_id;

        // Move the inventory to the player, it is logged by `handle_transfer_items`.
        transfer_items.write(TransferItems {
            instance_from_id: npc_id,
            instance_to_id: player_id,
        });

        // Drop the drawn weapon on the ground as a pickable item. Remove it
        // from the npc inventory first when present, otherwise the transfer
        // below would duplicate it.
        if let Some(item) = dropped {
            let mut script = gothic_scripts.lock();
            let removed = script.remove_inventory(npc_id, item, 1);
            if removed == 0 {
                warn!(
                    "Dropped weapon item({}) of npc({}) was not in its inventory",
                    item.value(),
                    npc_id.value()
                );
            }
            drop(script);
            match item_model_path(&gothic_scripts, &vfs, item) {
                Some(path) => {
                    let model_handle = handles.get_model_handle(&asset_server, &path, None);
                    commands.spawn((
                        ClassifyDistance::default(),
                        ZenGinModelComponent {
                            model_handle,
                            ..default()
                        },
                        Visibility::default(),
                        Transform::from_translation(position),
                        GameItem {
                            item_instance_id: item,
                        },
                    ));
                }
                None => warn!("No mesh path for dropped weapon item({item})"),
            }
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
