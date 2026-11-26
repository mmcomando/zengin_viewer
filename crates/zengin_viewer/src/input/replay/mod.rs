//! Replay of a scenario file, enabled by `--scenario`.
//!
//! A scenario ID is a path under `misc/scenario`, with or without the `.ron`
//! extension. Screenshot paths inside the file are resolved relative to
//! `misc/scenario_output/<id>/` and must not be absolute. `TestScreenshot`
//! additionally compares with (or, with `--update-reference`, copies into)
//! `misc/scenario_test_references/<id>/`.

mod actions;
mod command;
mod failures;
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

use crate::input::actions::update_input_mode;
use zengin_lib::gameplay::character_camera::camera_toggle_input;

use actions::replay_actions;
use command::apply_replay_locations;
use failures::{TestFailures, report_failures_on_exit};
use scenario::{finish_scenario, load_scenario};
use state::PendingLocations;

/// Replays the actions recorded with `--save-actions`.
pub struct ActionReplayPlugin;

impl Plugin for ActionReplayPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_scenario);
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
        app.add_systems(Update, finish_scenario);
        // Without `ExitGame` (frame limit) `finish_scenario` never runs its exit
        // path, so report collected test failures when an exit is requested.
        app.add_systems(Last, report_failures_on_exit);
        app.init_resource::<PendingLocations>();
        app.init_resource::<TestFailures>();
    }
}
