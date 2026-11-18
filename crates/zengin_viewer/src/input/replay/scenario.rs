use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy::render::view::window::screenshot::Screenshot;
use ron::Options as RonOptions;
use ron::extensions::Extensions;

use crate::input::options::Options;
use crate::input::record::{DEFAULT_FRAME_DT, ReplayCommand};

use super::state::ActionReplay;

/// Root directory of all scenario workdirs.
const SCENARIO_DIR: &str = "misc/scenario_output";

/// Resolution used for scenario runs.
const SCENARIO_WINDOW_RESOLUTION: (u32, u32) = (1024, 768);

/// A replay command with its resolved execution frame.
#[derive(Clone, Debug)]
pub(super) struct ScheduledCommand {
    pub(super) frame: u64,
    pub(super) command: ReplayCommand,
}

/// Where a scenario run reads its commands from and writes its outputs to.
struct ScenarioLayout {
    /// RON file with the replay commands.
    scenario_file: PathBuf,
    /// Directory relative screenshot paths resolve against, cleared on start.
    workdir: PathBuf,
    /// Reference screenshots of a test scenario, `None` for a plain replay.
    reference: Option<PathBuf>,
    /// Copy produced screenshots into the reference instead of comparing.
    update_reference: bool,
}

/// Workdir of the replay named by its file: contains all outputs (screenshots)
/// of the replay under `misc/scenario_output/<file stem>/`.
fn scenario_workdir(replay_path: &Path) -> PathBuf {
    let name = replay_path.file_stem().map_or_else(
        || "replay".to_string(),
        |stem| stem.to_string_lossy().into_owned(),
    );
    Path::new(SCENARIO_DIR).join(name)
}

/// Applies the scenario window settings for replay runs: hidden window with a
/// fixed resolution, so screenshots are reproducible.
pub(crate) fn apply_scenario_window_settings(window: &mut Window, replay_enabled: bool) {
    if !replay_enabled {
        return;
    }
    window.visible = false;
    window.resolution = SCENARIO_WINDOW_RESOLUTION.into();
}

/// Resolves the scenario layout from the command line options.
fn scenario_layout(options: &Options) -> Option<ScenarioLayout> {
    if let Some(dir) = options.test_scenario() {
        return Some(ScenarioLayout {
            scenario_file: dir.join("scenario.ron"),
            workdir: dir.join("workdir"),
            reference: Some(dir.join("reference")),
            update_reference: options.update_reference(),
        });
    }
    let path = options.replay_actions()?;
    Some(ScenarioLayout {
        scenario_file: path.clone(),
        workdir: scenario_workdir(path),
        reference: None,
        update_reference: false,
    })
}

/// Loads and prepares the scenario file named by the command line options.
///
/// Outputs of the replay (screenshots) resolve against the scenario workdir.
pub(super) fn load_replay_file(options: Res<Options>, mut commands: Commands) {
    let Some(layout) = scenario_layout(&options) else {
        return;
    };
    let replay = prepare_replay(&layout);
    info!(
        "Replaying {} recorded replay command(s) from {:?}",
        replay.commands.len(),
        layout.scenario_file
    );
    commands.insert_resource(replay);
}

/// Resets the scenario workdir so everything left in it was produced by
/// this run, not by an earlier one. The workdir itself is kept in place, only
/// its entries are removed, since editors and file watchers dislike watched
/// directories disappearing.
fn reset_scenario_workdir(workdir: &Path) {
    let entries = match fs::read_dir(workdir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            create_scenario_workdir(workdir);
            return;
        }
        Err(error) => {
            warn!("Failed to read scenario workdir {workdir:?}: {error}");
            return;
        }
    };
    for entry in entries {
        match entry {
            Ok(entry) => remove_scenario_output(&entry.path()),
            Err(error) => warn!("Failed to read entry of scenario workdir {workdir:?}: {error}"),
        }
    }
}

/// Creates the scenario workdir, warning when it cannot be created.
fn create_scenario_workdir(workdir: &Path) {
    if let Err(error) = fs::create_dir_all(workdir) {
        warn!("Failed to create scenario workdir {workdir:?}: {error}");
    }
}

/// Removes a single entry left from an earlier scenario run.
fn remove_scenario_output(path: &Path) {
    let result = if path.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    };
    if let Err(error) = result {
        warn!("Failed to remove old scenario output {path:?}: {error}");
    }
}

/// Resets the scenario workdir and parses the replay file.
///
/// Panics when the scenario file is missing, unreadable or malformed, since
/// that is a broken invocation rather than a degraded replay.
fn prepare_replay(layout: &ScenarioLayout) -> ActionReplay {
    if !layout.scenario_file.exists() {
        panic!(
            "Scenario file {} does not exist",
            layout.scenario_file.display()
        );
    }
    reset_scenario_workdir(&layout.workdir);
    match read_replay(layout) {
        Ok(replay) => replay,
        Err(error) => panic!("Failed to prepare scenario replay: {error}"),
    }
}

fn read_replay(layout: &ScenarioLayout) -> Result<ActionReplay, String> {
    let path = &layout.scenario_file;
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let commands: Vec<ReplayCommand> = RonOptions::default()
        .with_default_extension(Extensions::IMPLICIT_SOME)
        .from_str(&contents)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))?;
    let commands = schedule_commands(commands)?;
    Ok(ActionReplay {
        workdir: layout.workdir.clone(),
        reference: layout.reference.clone(),
        update_reference: layout.update_reference,
        commands,
        next: 0,
        frame: 0,
        exit_requested: false,
        held_buttons: HashSet::new(),
        started_buttons: HashSet::new(),
        released_buttons: HashSet::new(),
        held_axes: HashMap::new(),
    })
}

/// Resolves the absolute frame of every command. A command without `frame`
/// runs `frame_dt` after the previous command in file order, or
/// [`DEFAULT_FRAME_DT`] when it has no timing field at all.
pub(super) fn schedule_commands(
    commands: Vec<ReplayCommand>,
) -> Result<Vec<ScheduledCommand>, String> {
    let mut previous_frame: u64 = 0;
    let mut scheduled = Vec::with_capacity(commands.len());
    for (index, command) in commands.into_iter().enumerate() {
        let frame = match command.timing() {
            (Some(frame), None) => frame,
            (None, Some(delta)) => previous_frame.saturating_add(delta),
            (None, None) => previous_frame.saturating_add(DEFAULT_FRAME_DT),
            (Some(_), Some(_)) => {
                return Err(format!(
                    "command {index} {command:?} has both frame and frame_dt"
                ));
            }
        };
        previous_frame = frame;
        scheduled.push(ScheduledCommand { frame, command });
    }
    scheduled.sort_by_key(|scheduled| scheduled.frame);
    Ok(scheduled)
}

/// Exits a test scenario run after an explicit `ExitGame` once all screenshots
/// were captured and compared, so comparisons are never skipped by leaving
/// early. Plain replays exit through `ExitGame` or the frame limit.
pub(super) fn finish_test_scenario(
    replay: Option<Res<ActionReplay>>,
    screenshots: Query<(), With<Screenshot>>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(replay) = replay else {
        return;
    };
    if replay.reference.is_none() || !screenshots.is_empty() {
        return;
    }
    if !replay.exit_requested {
        return;
    }
    if replay.update_reference {
        info!("Test scenario finished, screenshots written to the reference directory");
    } else {
        info!("Test scenario finished, all screenshots match their references");
    }
    exit.write(AppExit::Success);
}
