use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use bevy::prelude::*;
use bevy::render::view::window::screenshot::Screenshot;
use ron::Options as RonOptions;
use ron::extensions::Extensions;

use crate::input::options::Options;
use crate::input::record::{DEFAULT_FRAME_DT, ReplayCommand};

use super::failures::TestFailures;
use super::state::ActionReplay;

/// Root directory of the scenario files.
const SCENARIO_ROOT: &str = "misc/scenario";
/// Root directory of the scenario workdirs.
const SCENARIO_OUTPUT_ROOT: &str = "misc/scenario_output";
/// Root directory of the reference screenshots compared by `TestScreenshot`.
const SCENARIO_REFERENCE_ROOT: &str = "misc/scenario_test_references";

/// Resolution used for scenario runs.
const SCENARIO_WINDOW_RESOLUTION: (u32, u32) = (1024, 768);

/// A replay command with its resolved execution frame.
#[derive(Clone, Debug)]
pub(super) struct ScheduledCommand {
    pub(super) frame: u64,
    pub(super) command: ReplayCommand,
}

/// Where a scenario run reads its commands from and writes its outputs to.
pub(super) struct ScenarioLayout {
    /// RON file with the replay commands.
    pub(super) scenario_file: PathBuf,
    /// Scenario ID without the `.ron` extension, e.g. `tests/dialog/lares`.
    pub(super) id: PathBuf,
    /// Directory relative screenshot paths resolve against, cleared on start.
    pub(super) workdir: PathBuf,
    /// Root directory of the `TestScreenshot` reference screenshots.
    pub(super) reference_root: PathBuf,
    /// Copy produced test screenshots into the reference instead of comparing.
    pub(super) update_reference: bool,
}

impl ScenarioLayout {
    /// Resolves a scenario ID (a path under [`SCENARIO_ROOT`], with or without
    /// the `.ron` extension) to its file, workdir and reference directory.
    pub(super) fn from_id(id: &Path, update_reference: bool) -> Self {
        let id = normalize_scenario_id(id);
        let mut scenario_file = Path::new(SCENARIO_ROOT).join(&id);
        if scenario_file.extension().is_none_or(|ext| ext != "ron") {
            scenario_file.set_extension("ron");
        }
        let id = scenario_file
            .strip_prefix(SCENARIO_ROOT)
            .map(Path::to_path_buf)
            .unwrap_or_else(|_| id.clone())
            .with_extension("");
        Self {
            workdir: Path::new(SCENARIO_OUTPUT_ROOT).join(&id),
            reference_root: Path::new(SCENARIO_REFERENCE_ROOT).join(&id),
            id,
            scenario_file,
            update_reference,
        }
    }
}

/// Rejects scenario IDs that would escape the scenario root. A path that
/// starts with the root itself is accepted, so both `tests/foo` and
/// `misc/scenario/tests/foo.ron` name the same scenario.
fn normalize_scenario_id(id: &Path) -> PathBuf {
    let id = id.strip_prefix(SCENARIO_ROOT).unwrap_or(id);
    if id.as_os_str().is_empty() {
        panic!("--scenario needs a scenario ID under {SCENARIO_ROOT}");
    }
    for component in id.components() {
        match component {
            Component::Normal(_) | Component::CurDir => {}
            _ => panic!(
                "--scenario {id:?} must be a relative path under {SCENARIO_ROOT} \
                 without `..` components"
            ),
        }
    }
    id.to_path_buf()
}

/// Applies the scenario window settings for scenario runs: hidden window with
/// a fixed resolution, so screenshots are reproducible.
pub(crate) fn apply_scenario_window_settings(window: &mut Window, scenario_enabled: bool) {
    if !scenario_enabled {
        return;
    }
    window.visible = false;
    window.resolution = SCENARIO_WINDOW_RESOLUTION.into();
}

/// Resolves the scenario layout from the command line options.
fn scenario_layout(options: &Options) -> Option<ScenarioLayout> {
    let id = options.scenario()?;
    Some(ScenarioLayout::from_id(id, options.update_reference()))
}

/// Loads and prepares the scenario file named by the command line options.
///
/// Outputs of the scenario (screenshots) resolve against the scenario workdir.
pub(super) fn load_scenario(options: Res<Options>, mut commands: Commands) {
    let Some(layout) = scenario_layout(&options) else {
        return;
    };
    let replay = prepare_replay(&layout, options.renderless());
    info!(
        "Replaying {} scenario command(s) from {:?} (id {:?})",
        replay.commands.len(),
        layout.scenario_file,
        layout.id
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

/// Resets the scenario workdir and parses the scenario file.
///
/// Panics when the scenario file is missing, unreadable or malformed, since
/// that is a broken invocation rather than a degraded replay.
fn prepare_replay(layout: &ScenarioLayout, renderless: bool) -> ActionReplay {
    if !layout.scenario_file.exists() {
        panic!(
            "Scenario file {} does not exist; scenario IDs are paths under {SCENARIO_ROOT}",
            layout.scenario_file.display()
        );
    }
    reset_scenario_workdir(&layout.workdir);
    match read_replay(layout, renderless) {
        Ok(replay) => replay,
        Err(error) => panic!("Failed to prepare scenario: {error}"),
    }
}

fn read_replay(layout: &ScenarioLayout, renderless: bool) -> Result<ActionReplay, String> {
    let path = &layout.scenario_file;
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let commands: Vec<ReplayCommand> = RonOptions::default()
        .with_default_extension(Extensions::IMPLICIT_SOME)
        .from_str(&contents)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))?;
    let commands = schedule_commands(commands);
    Ok(ActionReplay {
        workdir: layout.workdir.clone(),
        reference_root: layout.reference_root.clone(),
        update_reference: layout.update_reference,
        renderless,
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

/// Resolves the absolute frame of every command. Every command runs
/// [`DEFAULT_FRAME_DT`] after the previous one, except `Wait`, which runs
/// `frame_dt` after it.
pub(super) fn schedule_commands(commands: Vec<ReplayCommand>) -> Vec<ScheduledCommand> {
    let mut previous_frame: u64 = 0;
    let mut scheduled = Vec::with_capacity(commands.len());
    for command in commands {
        let delay = if let ReplayCommand::Wait { frame_dt } = &command {
            *frame_dt
        } else {
            DEFAULT_FRAME_DT
        };
        let frame = previous_frame.saturating_add(delay);
        previous_frame = frame;
        scheduled.push(ScheduledCommand { frame, command });
    }
    scheduled
}

/// Exits a scenario run after an explicit `ExitGame` once all pending
/// screenshots were captured and compared, so comparisons are never skipped by
/// leaving early. Collected test failures are reported here, at the exit of the
/// test, so the whole scenario and its logs run first. The frame limit exits a
/// run without `ExitGame`; failures are then reported by
/// [`report_failures_on_exit`](super::failures::report_failures_on_exit).
pub(super) fn finish_scenario(
    replay: Option<Res<ActionReplay>>,
    screenshots: Query<(), With<Screenshot>>,
    failures: Res<TestFailures>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(replay) = replay else {
        return;
    };
    if !replay.exit_requested {
        return;
    }
    if !screenshots.is_empty() {
        return;
    }
    failures.panic_if_any();
    if replay.update_reference {
        info!("Scenario finished, test screenshots written to the reference directory");
    } else {
        info!("Scenario finished");
    }
    exit.write(AppExit::Success);
}

#[cfg(test)]
mod tests {
    use super::ScenarioLayout;
    use std::path::{Path, PathBuf};

    fn layout(id: &str) -> ScenarioLayout {
        ScenarioLayout::from_id(Path::new(id), false)
    }

    #[test]
    fn nested_id_resolves_to_the_workdir_and_reference_dirs() {
        let layout = layout("tests/dialog/lares");
        assert_eq!(
            layout.scenario_file,
            PathBuf::from("misc/scenario/tests/dialog/lares.ron")
        );
        assert_eq!(layout.id, PathBuf::from("tests/dialog/lares"));
        assert_eq!(
            layout.workdir,
            PathBuf::from("misc/scenario_output/tests/dialog/lares")
        );
        assert_eq!(
            layout.reference_root,
            PathBuf::from("misc/scenario_test_references/tests/dialog/lares")
        );
    }

    #[test]
    fn ron_extension_and_root_prefix_are_optional() {
        let with_extension = layout("tests/dialog/lares.ron");
        let with_root = layout("misc/scenario/tests/dialog/lares.ron");
        assert_eq!(with_extension.scenario_file, with_root.scenario_file);
        assert_eq!(with_extension.id, with_root.id);
        assert_eq!(with_extension.workdir, with_root.workdir);
    }

    #[test]
    #[should_panic(expected = "must be a relative path")]
    fn parent_dir_is_rejected() {
        layout("../outside");
    }

    #[test]
    #[should_panic(expected = "must be a relative path")]
    fn absolute_path_is_rejected() {
        layout("/tmp/scenario");
    }
}
