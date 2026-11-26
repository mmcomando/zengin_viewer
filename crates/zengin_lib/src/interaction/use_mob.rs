//! Using the selected interactive mob.
//!
//! Mobs with a player pose (benches, chairs, beds, anvils, ...) attach the
//! player through the shared npc `UseMobAction`; the pose, occupancy, body
//! state and `onStateFunc` hook then come from the npc path. Everything else
//! (doors, levers, wheels) keeps the plain state toggle: `Interact` flips the
//! mob state, which plays its own animation and runs its `onStateFunc` hook.

use bevy::log::{info, warn};
use bevy::prelude::*;
use leafwing_input_manager::prelude::ActionState;

use super::selection::{Selection, SelectionKind};
use crate::PlayerCharacter;
use crate::gameplay::objects::NpcModel;
use crate::npc_actions::{UseMobAction, mob_scheme_has_player_pose};
use crate::npc_routines::{AnimationFiles, NpcOverlay, NpcWeaponMode};
use crate::script::component::ScriptResource;
use crate::script::manager::ScriptManager;
use crate::script::state::WeaponMode;
use crate::script::string_value::StringCanon;
use crate::script::symbol_id::InstanceId;
use crate::{GameMob, GameNpc, MobBase, MobPose, NpcMobUse, VobTrigger};
use dev_util::perf_debug;
use input_core::{GameplayAction, GameplayInputMarker};

/// Runs the mob's script-side use conditions and logs the reason when the
/// player is not allowed to interact.
fn mob_use_denied(
    script: &mut ScriptManager,
    player: InstanceId,
    target: Entity,
    mob: &GameMob,
) -> bool {
    if !script.mob_condition_allows(player, &mob.condition_func) {
        info!(
            "Player({player}) mob({target}) use denied by condition_func({})",
            mob.condition_func
        );
        return true;
    }
    if script.refuse_missing_mob_item(player, &mob.use_with_item) {
        info!(
            "Player({player}) mob({target}) use denied: missing item({})",
            mob.use_with_item
        );
        return true;
    }
    false
}

/// Notifies the mob's `triggerTarget` (mover, gate, ...) on use, unless it was
/// already fired and does not rewind.
fn fire_trigger(mob: &mut GameMob, vob_trigger: &mut MessageWriter<VobTrigger>) {
    if mob.trigger_target.is_empty() || (!mob.rewind && mob.trigger_fired) {
        return;
    }
    mob.trigger_fired = true;
    vob_trigger.write(VobTrigger {
        target: mob.trigger_target.clone(),
    });
}

/// Starts the player's use of the selected mob: attaches through the shared
/// `UseMobAction` when the mob has a player pose, toggles its state otherwise.
#[allow(
    clippy::cognitive_complexity,
    clippy::too_many_arguments,
    clippy::type_complexity
)]
pub fn use_selected_mob(
    mut commands: Commands,
    action_state: Single<&ActionState<GameplayAction>, With<GameplayInputMarker>>,
    selection: Res<Selection>,
    players: Query<
        (
            Entity,
            &GameNpc,
            Option<&NpcModel>,
            Option<&NpcOverlay>,
            Option<&NpcMobUse>,
        ),
        With<PlayerCharacter>,
    >,
    player_weapon: Query<&NpcWeaponMode, With<PlayerCharacter>>,
    mut mobs: Query<(&mut GameMob, &mut MobPose, &MobBase)>,
    animation_files: Option<Res<AnimationFiles>>,
    gothic_script: Res<ScriptResource>,
    mut vob_trigger: MessageWriter<VobTrigger>,
) {
    let _perf = perf_debug!();
    if !action_state.just_pressed(&GameplayAction::Interact) {
        return;
    }
    let weapon_mode = player_weapon
        .single()
        .map_or(WeaponMode::Fists, |mode| mode.0);
    if weapon_mode != WeaponMode::Fists {
        return;
    }
    if selection.kind != Some(SelectionKind::Mob) {
        return;
    }
    let Some(target) = selection.target else {
        return;
    };
    let Ok((player_entity, player_npc, player_model, player_overlay, player_mob_use)) =
        players.single()
    else {
        return;
    };
    // An attached player stands up through `stand_up_player_from_mob`.
    if player_mob_use.is_some() {
        return;
    }
    let Ok((mut mob, mut pose, base)) = mobs.get_mut(target) else {
        return;
    };
    if pose.state_num <= 0 {
        warn!(
            "Mob({target}) vob({}) has no interaction states",
            base.vob_name
        );
        return;
    }
    let player = player_npc.instance_id();
    {
        let mut script = gothic_script.lock();
        if mob_use_denied(&mut script, player, target, &mob) {
            return;
        }
    }

    // Regular pose mobs (benches, beds, anvils) stay attached in state 1 until
    // the player stands up. Switch-like vobs (levers, buttons, wheels, doors)
    // toggle their state and end automatically, like the engine's
    // `canQuitAtState`.
    let pose_state = if mob.auto_end {
        i32::from(pose.state <= 0)
    } else {
        1
    };
    let has_pose = match (player_model, animation_files.as_deref()) {
        (Some(model), Some(files)) => {
            mob_scheme_has_player_pose(model, player_overlay, files, &pose.scheme, pose_state)
        }
        _ => false,
    };
    if has_pose {
        fire_trigger(&mut mob, &mut vob_trigger);
        let scheme = StringCanon::new(&pose.scheme);
        let action = if mob.auto_end {
            UseMobAction::player_switch(target, scheme, pose_state)
        } else {
            UseMobAction::player(target, scheme, pose_state)
        };
        commands.entity(player_entity).insert(action);
        return;
    }

    let state = i32::from(pose.state <= 0);
    pose.state = state;
    fire_trigger(&mut mob, &mut vob_trigger);
    gothic_script
        .lock()
        .call_mob_state_hook(player, &mob.on_state_func, state);
}
