//! Replay of recorded commands from a RON file, enabled by `--replay-actions`
//! or `--test-scenario`.
//!
//! The file can live anywhere; paths inside it (screenshots) are resolved
//! relative to `misc/scenario_output/<file stem>/` for a plain replay, or to
//! `<test scenario dir>/workdir/` for a test scenario, and must not be absolute.
//! A test scenario additionally compares every screenshot with the same path
//! under `<test scenario dir>/reference/`.

mod actions;
mod command;
mod log_check;
mod scenario;
mod screenshot;
mod state;
mod world;

#[cfg(test)]
mod tests;

pub(crate) use scenario::apply_scenario_window_settings;

use bevy::prelude::*;
use leafwing_input_manager::plugin::InputManagerSystem;

use crate::gameplay::character_camera::camera_toggle_input;
use crate::input::actions::update_input_mode;

use actions::replay_actions;
use command::apply_replay_locations;
use scenario::{finish_test_scenario, load_replay_file};
use state::PendingLocations;

/// Replays the actions recorded with `--save-actions`.
pub struct ActionReplayPlugin;

impl Plugin for ActionReplayPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_replay_file);
        // Synthetic actions must be applied in the `ManualControl` set, after
        // the input manager recomputed the action states from raw device
        // input, otherwise they are overridden before consumers read them.
        app.add_systems(
            PreUpdate,
            replay_actions
                .in_set(InputManagerSystem::ManualControl)
                .after(update_input_mode),
        );
        // Location commands are applied after the camera toggle, so a camera
        // switch sync point overrides the toggle's placement at the drifted
        // free camera.
        app.add_systems(PreUpdate, apply_replay_locations.after(camera_toggle_input));
        app.add_systems(Update, finish_test_scenario);
        app.init_resource::<PendingLocations>();
    }
}
