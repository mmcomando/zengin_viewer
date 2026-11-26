//! Combat damage: melee swing hits, shared damage application and npc death
//! handling. The ranged counterpart lives in [`super::ranged`].

use bevy::prelude::*;

use super::selection::{Selection, SelectionKind};
use crate::gameplay::objects::NpcModel;
use crate::graphics::AnimationController;
use crate::graphics::{NpcHitReaction, play_hit_reaction};
use crate::gui::GameLog;
use crate::zengin::PlayerCharacter;
use crate::zengin::SpawnFromSavedObject;
use crate::zengin::instance_index::InstanceIndex;
use crate::zengin::npc_actions::{ExecutingActionMarker, NpcActionQueue};
use crate::zengin::npc_routines::NpcPath;
use crate::zengin::npc_routines::{
    AnimationFiles, NpcEquippedWeapon, NpcLookAt, NpcOverlay, NpcWeaponMode,
};
use crate::zengin::resources::CachedAssetHandles;
use crate::zengin::save::format::SavedObjectBuilder;
use crate::zengin::script::component::ScriptResource;
use crate::zengin::script::manager::AttackKind;
use crate::zengin::script::state::WeaponMode;
use crate::zengin::script::symbol_id::InstanceId;
use crate::zengin::{GameNpc, NpcCorpseMarker, VisibilityRefresh};
use dev_util::perf_debug;

/// Query data of the npc state needed to apply hits and deaths.
/// Shared with the ranged hit system in [`super::ranged`].
pub(super) type NpcHitQueryData = (
    &'static GameNpc,
    &'static GlobalTransform,
    &'static NpcModel,
    Option<&'static NpcOverlay>,
    Option<&'static mut AnimationController>,
    Option<&'static NpcHitReaction>,
    Has<PlayerCharacter>,
);

/// Query data of an npc entity on death.
type NpcDeathQueryData = (
    Option<&'static GameNpc>,
    Option<&'static NpcEquippedWeapon>,
    Option<&'static NpcWeaponMode>,
    Option<&'static mut NpcActionQueue>,
);

/// An npc died (hitpoints reached 0): its inventory stays on the corpse and the
/// drawn weapon drops on the ground.
#[derive(Message)]
pub struct NpcDied {
    pub entity: Entity,
    pub instance_id: InstanceId,
    /// Instance that dealt the killing hit, `None` for a script driven death
    /// without a last hitter.
    pub killer: Option<InstanceId>,
    /// Position where the dropped weapon is placed.
    pub position: Vec3,
}

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
/// [`MeleeHit`] arrives. A swing without a selected npc hits nothing. The npc
/// dies when its hitpoints reach 0.
pub(super) fn melee_swing_hit(
    mut commands: Commands,
    mut hits: MessageReader<MeleeHit>,
    selection: Res<Selection>,
    mut npcs: Query<NpcHitQueryData, Without<PlayerCharacter>>,
    player_npc: Query<&GameNpc, With<PlayerCharacter>>,
    gothic_script: Res<ScriptResource>,
    files: Res<AnimationFiles>,
    mut handles: ResMut<CachedAssetHandles>,
    asset_server: Res<AssetServer>,
    mut game_log: ResMut<GameLog>,
    mut npc_died: MessageWriter<NpcDied>,
) {
    let _perf = perf_debug!();
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
        let Ok((npc, global_tr, npc_model, overlay, controller, reaction, is_player)) =
            npcs.get_mut(target)
        else {
            continue;
        };
        let target_data = DamageTargetData {
            npc,
            global_tr,
            model: npc_model,
            overlay,
            controller,
            reaction,
            is_player,
        };
        let Ok(player_npc) = player_npc.single() else {
            continue;
        };
        let player_id = player_npc.instance_id();

        deal_damage(
            &mut commands,
            target,
            target_data,
            AttackKind::Melee,
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

/// Applies a melee hit of an npc to its `Npc_SetTarget` target. The player's
/// hitpoints are floored at one, it never dies.
#[allow(clippy::too_many_arguments, clippy::cognitive_complexity)]
pub(super) fn npc_melee_swing_hit(
    mut commands: Commands,
    mut hits: MessageReader<MeleeHit>,
    index: Res<InstanceIndex>,
    shooters: Query<(&GameNpc, &GlobalTransform), Without<PlayerCharacter>>,
    mut targets: Query<NpcHitQueryData>,
    transforms: Query<&GlobalTransform>,
    gothic_script: Res<ScriptResource>,
    files: Res<AnimationFiles>,
    mut handles: ResMut<CachedAssetHandles>,
    asset_server: Res<AssetServer>,
    mut game_log: ResMut<GameLog>,
    mut npc_died: MessageWriter<NpcDied>,
) {
    let _perf = perf_debug!();
    for hit in hits.read() {
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

        let Ok((target_npc, target_gt, model, overlay, controller, reaction, is_player)) =
            targets.get_mut(target_entity)
        else {
            warn!(
                "Npc({}) melee hit ignored: target({target_id}) is not a damageable entity",
                attacker_id.value()
            );
            continue;
        };
        let target_data = DamageTargetData {
            npc: target_npc,
            global_tr: target_gt,
            model,
            overlay,
            controller,
            reaction,
            is_player,
        };
        deal_damage(
            &mut commands,
            target_entity,
            target_data,
            AttackKind::Melee,
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

/// Damage application shared by every melee and ranged hit path, regardless
/// of whether the victim is the player or an npc. Changes the hitpoints, logs,
/// plays the `T_GOTHIT` hit reaction and emits `NpcDied` when an npc's
/// hitpoints reach 0. The player's hitpoints are floored at one, it never dies.
/// Logs a warn when the victim has no hitpoints data or either side has no
/// combat stats.
#[allow(clippy::too_many_arguments, clippy::cognitive_complexity)]
pub(crate) fn deal_damage(
    commands: &mut Commands,
    target: Entity,
    target_data: DamageTargetData,
    attack: AttackKind,
    shooter_id: InstanceId,
    gothic_script: &Res<ScriptResource>,
    files: &AnimationFiles,
    handles: &mut CachedAssetHandles,
    asset_server: &AssetServer,
    game_log: &mut GameLog,
    npc_died: &mut MessageWriter<NpcDied>,
) {
    let target_id = target_data.npc.instance_id();
    let (damage, hitpoints, shooter_is_player) = {
        let mut script = gothic_script.lock();
        if !target_data.is_player && script.npc_state(target_id).is_some_and(|npc| npc.live.dead) {
            return;
        }
        let shooter_is_player = script.player_instance() == Some(shooter_id);
        let Some(damage) = script.combat_damage(shooter_id, target_id, attack) else {
            warn!(
                "Damage ignored: no combat stats for attacker({}) or target({target_id})",
                shooter_id.value()
            );
            return;
        };
        let Some(hitpoints) = script.change_hitpoints(target_id, -damage) else {
            warn!(
                "Damage ignored: no hitpoints data for instance({})",
                target_id.value()
            );
            return;
        };
        script.npc_state_mut(target_id).live.last_hitter = Some(shooter_id);
        let hitpoints = if target_data.is_player && hitpoints < 1 {
            // The player never dies: the damage is applied but the hitpoints
            // floor at one.
            script
                .change_hitpoints(target_id, 1 - hitpoints)
                .unwrap_or(hitpoints)
        } else {
            hitpoints
        };
        if !target_data.is_player && hitpoints > 0 {
            script.run_damage_perception(target_id, shooter_id);
        }
        (damage, hitpoints, shooter_is_player)
    };
    if target_data.is_player {
        game_log.add(format!("Took {damage} damage"));
    } else if shooter_is_player {
        game_log.add(format!("Dealt {damage} damage"));
    }
    play_hit_reaction(
        commands,
        target,
        target_data.model,
        target_data.overlay,
        target_data.controller,
        target_data.reaction,
        files,
        handles,
        asset_server,
    );
    if !target_data.is_player && hitpoints <= 0 {
        npc_died.write(NpcDied {
            entity: target,
            instance_id: target_id,
            killer: Some(shooter_id),
            position: target_data.global_tr.translation(),
        });
    }
}

/// The target data [`deal_damage`] needs, gathered from the target query by the
/// caller (the query item is destructured into it). Carries `is_player` so the
/// same function applies hits to the player and to npcs.
pub(crate) struct DamageTargetData<'a> {
    pub npc: &'a GameNpc,
    pub global_tr: &'a GlobalTransform,
    pub model: &'a NpcModel,
    pub overlay: Option<&'a NpcOverlay>,
    pub controller: Option<Mut<'a, AnimationController>>,
    pub reaction: Option<&'a NpcHitReaction>,
    pub is_player: bool,
}

/// Removes the drawn weapon from the dead npc's inventory and queues it on the
/// ground as a pickable item.
fn drop_drawn_weapon(
    gothic_scripts: &ScriptResource,
    spawn_items: &mut MessageWriter<SpawnFromSavedObject>,
    npc_id: InstanceId,
    item: InstanceId,
    position: Vec3,
) {
    let removed = gothic_scripts.lock().remove_inventory(npc_id, item, 1);
    if removed == 0 {
        warn!(
            "Dropped weapon item({}) of npc({}) was not in its inventory",
            item.value(),
            npc_id.value()
        );
    }
    spawn_items.write(SpawnFromSavedObject {
        object: SavedObjectBuilder::new(Transform::from_translation(position))
            .item(item)
            .build(),
    });
}

/// A death that already ran the script death state and cleared the corpse's
/// equipment. The remaining jobs consume it in later systems.
#[derive(Message)]
pub(super) struct NpcDeathResolved {
    pub npc_id: InstanceId,
    pub position: Vec3,
    pub dropped_weapon: Option<InstanceId>,
}

/// Handles npc death: runs the script death state, clears the corpse's
/// equipment and marks the entity as a corpse. Dropping the drawn weapon is a
/// separate job fed by [`NpcDeathResolved`].
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
/// Forwards script driven deaths (`Npc_ChangeAttribute` reaching zero hitpoints)
/// into the regular death flow.
pub(super) fn script_npc_deaths(
    gothic_script: Res<ScriptResource>,
    index: Res<InstanceIndex>,
    transforms: Query<&GlobalTransform>,
    mut deaths: MessageWriter<NpcDied>,
) {
    let _perf = perf_debug!();
    let requests = gothic_script.lock().drain_npc_deaths();
    for (instance_id, killer) in requests {
        let Some(&entity) = index.npcs(instance_id).first() else {
            let mut script = gothic_script.lock();
            script.call_death_state(instance_id, killer);
            let mut state = script.npc_state_mut(instance_id);
            state.live.position = None;
            state.equipment.melee = None;
            state.equipment.ranged = None;
            state.equipment.weapon_mode = None;
            info!("Npc({}) died without an entity", instance_id.value());
            continue;
        };
        let position = transforms
            .get(entity)
            .map_or(Vec3::ZERO, |transform| transform.translation());
        deaths.write(NpcDied {
            entity,
            instance_id,
            killer,
            position,
        });
    }
}

pub(super) fn handle_npc_death(
    mut deaths: MessageReader<NpcDied>,
    mut resolved: MessageWriter<NpcDeathResolved>,
    mut visibility: MessageWriter<VisibilityRefresh>,
    mut selection: ResMut<Selection>,
    gothic_scripts: Res<ScriptResource>,
    mut commands: Commands,
    mut npcs: Query<NpcDeathQueryData, Without<PlayerCharacter>>,
) {
    let perf = perf_debug!();
    for death in deaths.read() {
        perf.increment_tag("deaths", 1);
        let Ok((npc, equipped, weapon_mode, queue)) = npcs.get_mut(death.entity) else {
            perf.increment_tag("missing_entity", 1);
            continue;
        };
        let npc_id = npc.map_or(death.instance_id, |npc| npc.instance_id());
        // Several hits can land on the same frame; only the first death
        // handling may run the state and move the inventory.
        if gothic_scripts
            .lock()
            .npc_state(npc_id)
            .is_some_and(|state| state.live.dead)
        {
            perf.increment_tag("already_dead", 1);
            continue;
        }
        // Run the script death state while the npc still has a position, then
        // clear it: script liveness checks read both flags.
        {
            let mut script = gothic_scripts.lock();
            script.call_death_state(npc_id, death.killer);
            let mut state = script.npc_state_mut(npc_id);
            state.live.position = None;
            // The corpse keeps its armor visual, but no weapon stays in its
            // hand: the drawn one drops and the rest stays in the inventory.
            state.equipment.melee = None;
            state.equipment.ranged = None;
            state.equipment.weapon_mode = None;
        }
        visibility.write(VisibilityRefresh);
        let position = death.position;
        // Only the drawn weapon drops on the ground; sheathed weapons stay in
        // the corpse inventory.
        let dropped = match weapon_mode.map_or(WeaponMode::Fists, |mode| mode.0) {
            WeaponMode::Melee => equipped.and_then(|equipped| equipped.melee),
            WeaponMode::Ranged => equipped.and_then(|equipped| equipped.ranged),
            WeaponMode::Fists => None,
        };

        perf.increment_tag("handled", 1);
        info!("Npc({}) died", npc_id);
        resolved.write(NpcDeathResolved {
            npc_id,
            position,
            dropped_weapon: dropped,
        });

        // The corpse stays in the world and keeps a death animation, but its
        // AI must not resume. `ExecutingActionMarker` removal also drops the
        // action components through the observer.
        if let Some(mut queue) = queue {
            queue.clear();
        }
        commands
            .entity(death.entity)
            .insert(NpcCorpseMarker)
            .remove::<(
                NpcPath,
                ExecutingActionMarker,
                NpcHitReaction,
                NpcEquippedWeapon,
                NpcWeaponMode,
                NpcLookAt,
            )>();
        if selection.target == Some(death.entity) {
            selection.target = None;
            selection.kind = None;
            selection.name.clear();
        }
    }
}

/// Drops the drawn weapon of a dead npc on the ground.
pub(super) fn drop_npc_death_weapon(
    gothic_scripts: Res<ScriptResource>,
    mut resolved: MessageReader<NpcDeathResolved>,
    mut spawn_items: MessageWriter<SpawnFromSavedObject>,
) {
    let _perf = perf_debug!();
    for death in resolved.read() {
        let Some(item) = death.dropped_weapon else {
            continue;
        };
        drop_drawn_weapon(
            &gothic_scripts,
            &mut spawn_items,
            death.npc_id,
            item,
            death.position,
        );
    }
}
