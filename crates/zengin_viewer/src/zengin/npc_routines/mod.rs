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
use crate::zengin::script::bridge::{ScriptChangesApplied, ScriptOutputFlush};
use crate::zengin::{ZenginVfs, loaders::vfs::animation_file_names};

use crate::perf_debug;
use crate::zengin::npc_actions::WaitAction;
use crate::zengin::npc_actions::{
    AttackAction, GoToNpcAction, GoToWaypointAction, StandUpAction, StartStateAction,
    UseItemStateAction, UseMobAction, advance_item_use_animations, advance_mob_use_pose,
    advance_npc_attack_swings, clear_npc_actions, drop_item_pose_for_action, end_attack_action,
    end_goto_action, end_goto_npc_action, end_use_mob_action, enqueue_align_actions,
    enqueue_attack_actions, enqueue_goto_actions, enqueue_goto_fp_actions,
    enqueue_goto_npc_actions, enqueue_stand_up_actions, enqueue_start_state_actions,
    enqueue_use_item_state_actions, enqueue_use_mob_actions, enqueue_wait_actions,
    flush_npc_action_queue, release_mob_use_actions, remove_actions_on_marker_removed,
    run_attack_action, run_goto_action, run_goto_npc_action, run_start_state_action,
    run_use_item_state_action, run_use_mob_action, run_wait_action, start_attack_action,
    start_goto_action, start_goto_npc_action, start_stand_up_action, start_start_state_action,
    start_use_mob_action, sync_npc_item_body_state, warn_direct_action_removal,
};
use armor::{materialize_armor_visuals, sync_npc_armor_visual};
use routines::{
    NpcRoutineCheckState, apply_look_at, apply_turn_to_npc, end_removed_routine_entries,
    move_along_path, pause_talking_npc_routine, release_npc_freepoints, restore_routine_stand,
    run_routine_loops, start_npc_start_ai_states, sync_script_freepoints,
    update_npc_routine_entries, update_npc_routines, update_routine_executors,
};
use script_animation::apply_scripted_animations;
use weapons::{
    apply_weapon_mode, update_npc_attachments, update_npc_item_use_items, update_npc_mob_use_items,
};

pub(crate) use armor::{NpcArmorItem, NpcBodyMarker};
pub(crate) use health::NpcHealth;
pub(crate) use overlay::overlay_scheme;
pub(crate) use overlay::{AnimationFiles, NpcOverlay};
pub(crate) use routines::{
    NPC_BODY_Y_OFFSET, NpcLookAt, NpcPath, NpcRoutineState, NpcWalkMode, RoutineExecutor,
    WayPoints, has_animation, queue_named_animation, queue_root_motion_animation,
    queue_stand_animation, start_ai_state,
};
pub(crate) use weapons::{NpcEquippedWeapon, NpcWeaponMode, NpcWeaponSlots};

/// Drives npc routines, queued actions, look-at and weapon state.
pub struct NpcRoutinesPlugin;

impl Plugin for NpcRoutinesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AnimationFiles>();
        app.add_systems(Startup, load_animation_files);
        app.init_resource::<NpcRoutineCheckState>();
        // State hooks queue their actions through the script bridge; flush those
        // requests before the action enqueue systems so the busy predicate sees
        // them in the update the state started.
        app.configure_sets(
            Update,
            ScriptOutputFlush
                .after(update_routine_executors)
                .after(start_npc_start_ai_states)
                .after(end_removed_routine_entries),
        );
        app.add_observer(remove_actions_on_marker_removed);
        app.add_systems(
            Update,
            (
                warn_direct_action_removal::<GoToWaypointAction>,
                warn_direct_action_removal::<GoToNpcAction>,
                warn_direct_action_removal::<AttackAction>,
                warn_direct_action_removal::<UseMobAction>,
                warn_direct_action_removal::<UseItemStateAction>,
                warn_direct_action_removal::<WaitAction>,
                warn_direct_action_removal::<StandUpAction>,
                warn_direct_action_removal::<StartStateAction>,
            ),
        );
        app.add_systems(
            Update,
            (
                (update_npc_routine_entries, update_npc_routines).chain(),
                clear_npc_actions,
                start_npc_start_ai_states.after(update_routine_executors),
                end_removed_routine_entries.after(update_npc_routine_entries),
                update_routine_executors.after(update_npc_routine_entries),
                run_routine_loops
                    .after(update_routine_executors)
                    .after(end_removed_routine_entries)
                    // The state start hook queues its first action through the
                    // script bridge; run the loop after that action is queued so
                    // the busy predicate sees it. Otherwise a state whose start
                    // hook moves the npc (zs_mm_hunt) ends on its first loop.
                    .after(enqueue_wait_actions),
                apply_scripted_animations,
                restore_routine_stand
                    .after(update_routine_executors)
                    .after(run_routine_loops)
                    .after(end_removed_routine_entries)
                    .after(apply_scripted_animations)
                    // A stale item-pose restore must not win over the stand reset
                    // when the state (and the item use) ends.
                    .after(advance_item_use_animations),
                (
                    (
                        release_mob_use_actions.before(flush_npc_action_queue),
                        enqueue_start_state_actions,
                        enqueue_stand_up_actions,
                        enqueue_goto_actions,
                        enqueue_goto_fp_actions,
                        enqueue_goto_npc_actions,
                        enqueue_align_actions,
                        enqueue_attack_actions,
                        enqueue_use_mob_actions,
                        enqueue_use_item_state_actions,
                        enqueue_wait_actions,
                        pause_talking_npc_routine,
                    )
                        .chain()
                        .after(clear_npc_actions),
                    (
                        end_goto_action,
                        end_goto_npc_action,
                        end_attack_action,
                        end_use_mob_action,
                    ),
                    flush_npc_action_queue,
                    (
                        drop_item_pose_for_action,
                        start_goto_action,
                        start_goto_npc_action,
                        start_attack_action,
                        start_use_mob_action,
                        start_stand_up_action,
                        start_start_state_action,
                    ),
                    (
                        run_goto_action,
                        run_goto_npc_action,
                        run_attack_action,
                        run_use_mob_action,
                        run_use_item_state_action,
                        run_wait_action,
                        run_start_state_action,
                    )
                        .before(AutoAnimSequencerSet),
                    advance_npc_attack_swings.after(AutoAnimSequencerSet),
                )
                    // Enqueue -> end -> flush -> start -> run. `chain` inserts the
                    // deferred-command sync points the transitions rely on.
                    .chain()
                    .after(update_npc_routines)
                    .after(clear_npc_actions)
                    .after(ScriptOutputFlush),
                (apply_turn_to_npc, apply_look_at)
                    .chain()
                    .after(update_npc_routines)
                    .after(ScriptOutputFlush),
                apply_weapon_mode.after(apply_scripted_animations),
                update_npc_attachments,
                update_npc_mob_use_items,
                update_npc_item_use_items,
                (sync_npc_armor_visual, materialize_armor_visuals).chain(),
                move_along_path
                    .after(run_goto_action)
                    .after(run_attack_action)
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
        app.add_systems(
            Update,
            advance_item_use_animations
                .after(run_use_item_state_action)
                .after(AutoAnimSequencerSet)
                .run_if(in_state(AppState::InGame)),
        );
        app.add_systems(
            Update,
            advance_mob_use_pose
                .after(run_use_mob_action)
                .after(AutoAnimSequencerSet)
                .run_if(in_state(AppState::InGame)),
        );
        app.add_systems(
            Update,
            sync_npc_item_body_state
                .after(ScriptChangesApplied)
                .run_if(in_state(AppState::InGame)),
        );
    }
}

/// Fills the anim archive inventory from the vfs once. It does not depend on a
/// loaded world, so scenarios that spawn npcs without one still resolve their
/// scripted animations.
fn load_animation_files(vfs: Res<ZenginVfs>, mut files: ResMut<AnimationFiles>) {
    let _perf = perf_debug!();
    files.0 = animation_file_names(&vfs);
}
