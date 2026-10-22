//! Npc routine driving: waypoint routines, look-at and weapon state changes
//! produced by the script vm, weapon mesh attachment and path following.

mod health;
mod overlay;
mod routines;
mod weapons;

use bevy::prelude::*;

use crate::gameplay::game_time::advance_game_time;
use crate::zengin::script::bridge::ScriptChangesApplied;

use routines::{
    NpcRoutineCheckState, apply_align_to_wp, apply_look_at, move_along_path, resolve_goto_paths,
    update_npc_routine_animations, update_npc_routine_entries, update_npc_routines,
};
use weapons::{apply_weapon_mode, update_npc_weapon};

pub use health::NpcHealth;
pub(crate) use overlay::overlay_scheme;
pub use overlay::{AnimationFiles, NpcOverlay};
pub use routines::{NpcLookAt, WayPoints};
pub use weapons::{NpcEquippedWeapon, NpcWeaponMode, NpcWeaponSlots};

/// Periodically teleports npcs to the waypoints of their active routine entries.
pub struct NpcRoutinesPlugin;

impl Plugin for NpcRoutinesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NpcRoutineCheckState>();
        app.add_systems(
            Update,
            (
                (update_npc_routine_entries, update_npc_routines).chain(),
                resolve_goto_paths,
                update_npc_routine_animations.after(update_npc_routines),
                apply_look_at.after(update_npc_routines),
                apply_align_to_wp.after(apply_look_at),
                apply_weapon_mode.after(update_npc_routine_animations),
                update_npc_weapon,
                move_along_path.after(update_npc_routines),
            )
                .after(advance_game_time)
                .after(ScriptChangesApplied),
        );
    }
}
