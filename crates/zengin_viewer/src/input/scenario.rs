use std::fs;

use bevy::prelude::*;
use bevy::render::view::window::screenshot::{Screenshot, save_to_disk};

use crate::input::options::Options;

/// Registers scenario specific resources and systems.
pub struct ScenarioPlugin {
    scenario: Scenario,
}

impl ScenarioPlugin {
    pub fn new(scenario: Scenario) -> Self {
        Self { scenario }
    }
}

impl Plugin for ScenarioPlugin {
    fn build(&self, app: &mut App) {
        match self.scenario {
            Scenario::MainTest => {
                reset_scenario_output_dir(Scenario::MainTest);
                app.insert_resource(FrameCounter(0));
                app.add_systems(Update, (take_screenshot_at_frame, exit_after_frame_limit));
            }
            Scenario::Normal => {}
        }
    }
}

pub const SCENARIO_OUTPUT_DIR: &str = "misc/scenario_output";

/// Resolution used for scenario runs.
const SCENARIO_WINDOW_RESOLUTION: (u32, u32) = (512, 512);

/// Applies scenario window settings: runs headless with a hidden window of
/// fixed resolution. The window still exists so the renderer and window
/// targeted screenshots keep working.
pub fn apply_scenario_window_settings(window: &mut Window, scenario: Scenario) {
    if scenario == Scenario::Normal {
        return;
    }
    window.visible = false;
    window.resolution = SCENARIO_WINDOW_RESOLUTION.into();
}

/// Predefined scenarios selectable via `--run-scenario`.
#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq)]
pub enum Scenario {
    /// Normal game loop, exits only when the window is closed.
    Normal,
    /// `main-test`: run 1000 frames, then exit.
    MainTest,
}

impl Scenario {
    /// Name used in logs and output file names (clap kebab-case name).
    fn name(self) -> &'static str {
        use clap::ValueEnum;
        self.to_possible_value()
            .map(|value| value.get_name().to_owned().leak() as &'static str)
            .unwrap_or("unknown")
    }
}

fn scenario_dir(scenario: Scenario) -> String {
    format!("{SCENARIO_OUTPUT_DIR}/{}", scenario.name())
}

/// Removes old artifacts of the scenario and recreates its output directory.
fn reset_scenario_output_dir(scenario: Scenario) {
    let dir = scenario_dir(scenario);
    match fs::remove_dir_all(&dir) {
        Ok(()) => info!("Removed old scenario output dir {dir}"),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => {
            warn!("Failed to remove old scenario output dir {dir}: {e}");
        }
    }
    if let Err(e) = fs::create_dir_all(&dir) {
        warn!("Failed to create scenario output dir {dir}: {e}");
    }
}

/// Counts simulated frames and exits the app when the scenario limit is
/// reached. Exits through the normal runner, so windows and the renderer are
/// fully initialized for the whole run.
#[derive(Resource)]
struct FrameCounter(u32);

const MAIN_TEST_FRAME_LIMIT: u32 = 500;
const MAIN_TEST_SCREENSHOT_FRAME: u32 = 200;

fn exit_after_frame_limit(mut counter: ResMut<FrameCounter>, mut exit: MessageWriter<AppExit>) {
    counter.0 += 1;
    if counter.0 % 100 == 0 {
        info!("run-scenario main-test: frame {}", counter.0);
    }
    if counter.0 >= MAIN_TEST_FRAME_LIMIT {
        info!("run-scenario main-test finished after {MAIN_TEST_FRAME_LIMIT} frames");
        exit.write(AppExit::Success);
    }
}

fn take_screenshot_at_frame(
    counter: Res<FrameCounter>,
    options: Res<Options>,
    mut commands: Commands,
) {
    if counter.0 != MAIN_TEST_SCREENSHOT_FRAME {
        return;
    }
    let path = format!(
        "{}/frame_{MAIN_TEST_SCREENSHOT_FRAME}_screenshot.png",
        scenario_dir(options.scenario()),
    );
    // Saving happens asynchronously once the capture is ready, `save_to_disk`
    // reports success/failure in its own logs.
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path.clone()));
    info!("Scenario screenshot requested: {path}");
}
