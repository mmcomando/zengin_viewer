//! Npc routine driving: waypoint routines, look-at and weapon state changes
//! produced by the script vm, weapon mesh attachment and path following.

mod armor;
mod health;
mod overlay;
mod routines;
mod script_animation;
mod weapons;

use bevy::prelude::*;

use crate::game_state::AppState;
use crate::gameplay::game_time::advance_game_time;
use crate::graphics::AutoAnimSequencerSet;
use crate::zengin::script::bridge::ScriptChangesApplied;

use crate::zengin::npc_actions::{
    advance_npc_attack_swings, clear_npc_actions, enqueue_align_actions, enqueue_attack_actions,
    enqueue_goto_actions, enqueue_goto_fp_actions, enqueue_use_mob_actions, run_npc_actions,
};
use armor::{materialize_armor_visuals, sync_npc_armor_visual};
use routines::{
    NpcRoutineCheckState, apply_look_at, apply_start_state_changes, apply_turn_to_npc,
    end_removed_routine_entries, move_along_path, release_npc_freepoints, restore_routine_stand,
    run_routine_loops, sync_script_freepoints, update_npc_routine_entries, update_npc_routines,
    update_routine_executors,
};
use script_animation::apply_scripted_animations;
use weapons::{apply_weapon_mode, update_npc_attachments};

pub(crate) use armor::{NpcArmorItem, NpcBodyMarker};
pub(crate) use health::NpcHealth;
pub(crate) use overlay::overlay_scheme;
pub(crate) use overlay::{AnimationFiles, NpcOverlay};
pub(crate) use routines::{
    NPC_BODY_Y_OFFSET, NpcLookAt, NpcPath, NpcRoutineState, WayPoints, queue_named_animation,
    queue_stand_animation,
};
pub(crate) use weapons::{NpcEquippedWeapon, NpcWeaponMode, NpcWeaponSlots};

/// Drives npc routines, queued actions, look-at and weapon state.
pub struct NpcRoutinesPlugin;

impl Plugin for NpcRoutinesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NpcRoutineCheckState>();
        app.add_systems(
            Update,
            (
                (update_npc_routine_entries, update_npc_routines).chain(),
                clear_npc_actions,
                apply_start_state_changes.after(clear_npc_actions),
                end_removed_routine_entries.after(update_npc_routine_entries),
                update_routine_executors
                    .after(update_npc_routine_entries)
                    .after(apply_start_state_changes),
                run_routine_loops
                    .after(update_routine_executors)
                    .after(end_removed_routine_entries),
                apply_scripted_animations,
                restore_routine_stand
                    .after(update_routine_executors)
                    .after(run_routine_loops)
                    .after(end_removed_routine_entries)
                    .after(apply_start_state_changes)
                    .after(apply_scripted_animations),
                enqueue_goto_actions.after(clear_npc_actions),
                enqueue_goto_fp_actions.after(clear_npc_actions),
                enqueue_align_actions.after(clear_npc_actions),
                enqueue_attack_actions.after(clear_npc_actions),
                enqueue_use_mob_actions.after(clear_npc_actions),
                run_npc_actions
                    .after(update_npc_routines)
                    .after(clear_npc_actions)
                    .after(enqueue_goto_actions)
                    .after(enqueue_goto_fp_actions)
                    .after(enqueue_align_actions)
                    .after(enqueue_attack_actions)
                    .after(enqueue_use_mob_actions),
                advance_npc_attack_swings
                    .after(run_npc_actions)
                    .after(AutoAnimSequencerSet),
                (apply_turn_to_npc, apply_look_at)
                    .chain()
                    .after(update_npc_routines),
                apply_weapon_mode.after(apply_scripted_animations),
                update_npc_attachments,
                (sync_npc_armor_visual, materialize_armor_visuals).chain(),
                move_along_path
                    .after(run_npc_actions)
                    .after(apply_scripted_animations)
                    .before(AutoAnimSequencerSet),
            )
                .after(advance_game_time)
                .after(ScriptChangesApplied)
                .run_if(in_state(AppState::InGame)),
        );
        app.add_systems(
            Update,
            release_npc_freepoints
                .after(update_routine_executors)
                .after(run_routine_loops)
                .after(advance_game_time)
                .after(ScriptChangesApplied)
                .run_if(in_state(AppState::InGame)),
        );
        app.add_systems(
            Update,
            sync_script_freepoints
                .after(ScriptChangesApplied)
                .run_if(in_state(AppState::InGame)),
        );
    }
}
