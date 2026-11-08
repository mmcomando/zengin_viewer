//! Npc routine driving: waypoint routines, look-at and weapon state changes
//! produced by the script vm, weapon mesh attachment and path following.

mod armor;
mod health;
mod overlay;
mod routines;
mod weapons;

use bevy::prelude::*;

use crate::game_state::AppState;
use crate::gameplay::game_time::advance_game_time;
use crate::zengin::script::bridge::ScriptChangesApplied;

use crate::zengin::npc_actions::{enqueue_align_actions, enqueue_goto_actions, run_npc_actions};
use armor::{materialize_armor_visuals, sync_npc_armor_visual};
use routines::{
    NpcRoutineCheckState, apply_look_at, move_along_path, update_npc_routine_animations,
    update_npc_routine_entries, update_npc_routines,
};
use weapons::{apply_weapon_mode, update_npc_attachments};

pub(crate) use armor::{NpcArmorItem, NpcBodyMarker};
pub(crate) use health::NpcHealth;
pub(crate) use overlay::overlay_scheme;
pub(crate) use overlay::{AnimationFiles, NpcOverlay};
pub(crate) use routines::{
    NPC_BODY_Y_OFFSET, NPC_WALK_SPEED, NpcLookAt, NpcPath, NpcRoutineState, WayPoints,
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
                enqueue_goto_actions,
                enqueue_align_actions,
                run_npc_actions
                    .after(update_npc_routines)
                    .after(enqueue_goto_actions)
                    .after(enqueue_align_actions),
                update_npc_routine_animations.after(update_npc_routines),
                apply_look_at.after(update_npc_routines),
                apply_weapon_mode.after(update_npc_routine_animations),
                update_npc_attachments,
                (sync_npc_armor_visual, materialize_armor_visuals).chain(),
                move_along_path
                    .after(run_npc_actions)
                    .after(update_npc_routine_animations),
            )
                .after(advance_game_time)
                .after(ScriptChangesApplied)
                .run_if(in_state(AppState::InGame)),
        );
    }
}
