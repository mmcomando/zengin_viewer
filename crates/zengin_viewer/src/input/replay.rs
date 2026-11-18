//! Replay of recorded commands from a RON file, enabled by `--replay-actions`
//! or `--test-scenario`.
//!
//! The file can live anywhere; paths inside it (screenshots) are resolved
//! relative to `misc/scenario_output/<file stem>/` for a plain replay, or to
//! `<test scenario dir>/workdir/` for a test scenario, and must not be absolute.
//! A test scenario additionally compares every screenshot with the same path
//! under `<test scenario dir>/reference/`.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy::render::view::window::screenshot::{Screenshot, ScreenshotCaptured};
use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView, ImageFormat};
use leafwing_input_manager::action_state::ActionKindData;
use leafwing_input_manager::buttonlike::ButtonState;
use leafwing_input_manager::plugin::InputManagerSystem;
use leafwing_input_manager::prelude::{ActionState, Actionlike};
use regex::Regex;
use ron::Options as RonOptions;
use ron::extensions::Extensions;

use crate::game_state::AppState;
use crate::gameplay::character_camera::camera_toggle_input;
use crate::gameplay::game_time::GameTime;
use crate::gameplay::objects::PlayerCharacter;
use crate::gui::GameLog;
use crate::input::actions::{
    EditorAction, EditorInputMarker, GameplayAction, GameplayInputMarker, GuiAction,
    GuiInputMarker, SharedAction, SharedInputMarker, update_input_mode,
};
use crate::input::log_capture::LogBuffer;
use crate::input::options::Options;
use crate::input::record::{DEFAULT_FRAME_DT, RecordedAction, ReplayCommand};
use crate::zengin::{
    DebugNpc, GameNpc, NpcSpawnNameFilter, ScriptResource, SpawnPositionFilter, WeaponKind,
    grant_test_items, seed_test_journal,
};

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

/// Root directory of all scenario workdirs.
const SCENARIO_DIR: &str = "misc/scenario_output";

/// Resolution used for scenario runs.
const SCENARIO_WINDOW_RESOLUTION: (u32, u32) = (1024, 768);

/// Size the screenshots are downscaled to before comparison. Coarse enough to
/// ignore anti-aliasing and GPU noise, fine enough to catch UI and lighting
/// changes.
const COMPARE_GRID: (u32, u32) = (64, 48);

/// Mean absolute channel difference (in `0..=1`) a grid cell needs to reach to
/// count as changed.
const COMPARE_CELL_THRESHOLD: f32 = 16.0 / 255.0;

/// Fraction of changed grid cells that is still accepted as the same image.
const COMPARE_MAX_CHANGED_FRACTION: f32 = 0.02;

/// Number of trailing log lines reported when `LogExists` does not match.
const LOG_EXISTS_TAIL_LINES: usize = 50;

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
pub fn apply_scenario_window_settings(window: &mut Window, replay_enabled: bool) {
    if !replay_enabled {
        return;
    }
    window.visible = false;
    window.resolution = SCENARIO_WINDOW_RESOLUTION.into();
}

/// A replay command with its resolved execution frame.
#[derive(Clone, Debug)]
struct ScheduledCommand {
    frame: u64,
    command: ReplayCommand,
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

/// Replay schedule loaded from the scenario file plus the replay state.
#[derive(Resource)]
struct ActionReplay {
    /// Workdir used to resolve relative paths inside the commands.
    workdir: PathBuf,
    /// Reference screenshots to compare with, `None` for a plain replay.
    reference: Option<PathBuf>,
    /// Copy produced screenshots into the reference instead of comparing.
    update_reference: bool,
    /// Commands with resolved frames, sorted by frame.
    commands: Vec<ScheduledCommand>,
    /// Index of the next event to apply.
    next: usize,
    frame: u64,
    /// `ExitGame` was requested but the test run still has to finish capturing
    /// and comparing screenshots before the app can exit.
    exit_requested: bool,
    /// Buttons currently held by the replay.
    held_buttons: HashSet<RecordedAction>,
    /// Buttons whose press event was processed this frame; these get a real
    /// `JustPressed` press, later held frames only keep the pressed state.
    started_buttons: HashSet<RecordedAction>,
    /// Latest dual axis values applied by the replay.
    held_axes: HashMap<RecordedAction, Vec2>,
}

/// Loads and prepares the scenario file named by the command line options.
///
/// Outputs of the replay (screenshots) resolve against the scenario workdir.
fn load_replay_file(options: Res<Options>, mut commands: Commands) {
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
        held_axes: HashMap::new(),
    })
}

/// Resolves the absolute frame of every command. A command without `frame`
/// runs `frame_dt` after the previous command in file order, or
/// [`DEFAULT_FRAME_DT`] when it has no timing field at all.
fn schedule_commands(commands: Vec<ReplayCommand>) -> Result<Vec<ScheduledCommand>, String> {
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

/// Teleport and camera location commands of the current frame, buffered by
/// [`replay_actions`] and applied by [`apply_replay_locations`].
#[derive(Resource, Default)]
struct PendingLocations {
    player: Vec<(Vec3, Vec3)>,
    camera: Vec<(Vec3, Vec3)>,
}

/// Applies buffered replay location commands after the camera toggle, so a
/// camera switch sync point overrides the toggle's placement at the drifted
/// free camera.
fn apply_replay_locations(
    mut pending: ResMut<PendingLocations>,
    mut player: Query<&mut Transform, (With<PlayerCharacter>, Without<Camera3d>)>,
    mut camera: Query<&mut Transform, (With<Camera3d>, Without<PlayerCharacter>)>,
) {
    for (translation, rotation) in pending.player.drain(..) {
        teleport_player(&mut player, translation, rotation);
    }
    for (translation, rotation) in pending.camera.drain(..) {
        set_camera_location(&mut camera, translation, rotation);
    }
}

/// Applies the replayed actions of the current frame.
///
/// Held buttons and axes are re-applied every frame, because the input
/// manager recomputes action states from raw device input each frame and
/// would otherwise wipe synthetic changes immediately.
fn replay_actions(
    mut commands: Commands,
    replay: Option<ResMut<ActionReplay>>,
    mut gameplay: Query<&mut ActionState<GameplayAction>, With<GameplayInputMarker>>,
    mut gui: Query<&mut ActionState<GuiAction>, With<GuiInputMarker>>,
    mut shared: Query<&mut ActionState<SharedAction>, With<SharedInputMarker>>,
    mut editor: Query<&mut ActionState<EditorAction>, With<EditorInputMarker>>,
    gothic_script: Option<Res<ScriptResource>>,
    player_npc: Query<&GameNpc, With<PlayerCharacter>>,
    mut game_time: ResMut<GameTime>,
    mut pending: ResMut<PendingLocations>,
    mut game_log: ResMut<GameLog>,
    mut next_state: ResMut<NextState<AppState>>,
    mut npc_filter: ResMut<NpcSpawnNameFilter>,
    mut spawn_filter: ResMut<SpawnPositionFilter>,
    log_buffer: Option<Res<LogBuffer>>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(mut replay) = replay else {
        return;
    };
    replay.frame += 1;
    replay.started_buttons.clear();

    while replay.next < replay.commands.len() && replay.commands[replay.next].frame <= replay.frame
    {
        let command = replay.commands[replay.next].command.clone();
        replay.next += 1;
        info!("Replay triggered command: {command:?}");
        if apply_replay_command(
            &mut replay,
            &command,
            gothic_script.as_deref(),
            &player_npc,
            &mut game_time,
            &mut pending,
            &mut commands,
            &mut game_log,
            &mut next_state,
            &mut npc_filter,
            &mut spawn_filter,
            log_buffer.as_deref(),
            &mut exit,
        ) {
            commands.remove_resource::<ActionReplay>();
            return;
        }
    }

    for action in &replay.held_buttons {
        let started = replay.started_buttons.contains(action);
        press(
            &mut gameplay,
            &mut gui,
            &mut shared,
            &mut editor,
            *action,
            started,
        );
    }
    for (action, value) in &replay.held_axes {
        set_axis_pair(&mut gameplay, *action, *value);
    }
}

/// Applies one replayed command. Returns `true` when the replay must abort
/// (an invalid screenshot path disables the replay entirely).
#[allow(clippy::cognitive_complexity)]
fn apply_replay_command(
    replay: &mut ActionReplay,
    command: &ReplayCommand,
    gothic_script: Option<&ScriptResource>,
    player_npc: &Query<&GameNpc, With<PlayerCharacter>>,
    game_time: &mut GameTime,
    pending: &mut PendingLocations,
    commands: &mut Commands,
    game_log: &mut GameLog,
    next_state: &mut NextState<AppState>,
    npc_filter: &mut NpcSpawnNameFilter,
    spawn_filter: &mut SpawnPositionFilter,
    log_buffer: Option<&LogBuffer>,
    exit: &mut MessageWriter<AppExit>,
) -> bool {
    match command {
        ReplayCommand::Press { action, .. } => {
            replay.started_buttons.insert(*action);
            replay.held_buttons.insert(*action);
        }
        ReplayCommand::Release { action, .. } => {
            replay.held_buttons.remove(action);
        }
        ReplayCommand::Axis { action, value, .. } => {
            if *value == Vec2::ZERO {
                replay.held_axes.remove(action);
            } else {
                replay.held_axes.insert(*action, *value);
            }
        }
        ReplayCommand::TeleportPlayer {
            translation,
            rotation,
            ..
        } => {
            pending.player.push((*translation, *rotation));
        }
        ReplayCommand::SetCameraLocation {
            translation,
            rotation,
            ..
        } => {
            pending.camera.push((*translation, *rotation));
        }
        ReplayCommand::ZenginSetGameTimeSpeed { speed, .. } => {
            game_time.speed = *speed;
            info!("Replay set the in-game time speed multiplier to {speed}");
        }
        ReplayCommand::ZenginEquipWeapon { item, .. } => {
            let Some(gothic_script) = gothic_script else {
                warn!("ZenginEquipWeapon replay command: game script is not loaded");
                return false;
            };
            equip_player_weapon(gothic_script, player_npc, item);
        }
        ReplayCommand::ZenginToggleArmor { item, .. } => {
            let Some(gothic_script) = gothic_script else {
                warn!("ZenginToggleArmor replay command: game script is not loaded");
                return false;
            };
            toggle_player_armor(gothic_script, player_npc, item);
        }
        ReplayCommand::Screenshot { path, .. } => {
            return apply_replay_screenshot(replay, path, commands);
        }
        ReplayCommand::ZenginGiveTestItems { .. } => {
            let Some(gothic_script) = gothic_script else {
                warn!("ZenginGiveTestItems replay command: game script is not loaded");
                return false;
            };
            let Ok(player_npc) = player_npc.single() else {
                warn!("ZenginGiveTestItems replay command: player npc is not spawned");
                return false;
            };
            let player_id = player_npc.instance_id();
            let mut script = gothic_script.lock();
            let lines = grant_test_items(&mut script, player_id);
            if !lines.is_empty() {
                game_log.add(lines.join("\n"));
            }
            info!(
                "Replay granted {} test item stack(s) to player({})",
                lines.len(),
                player_id
            );
        }
        ReplayCommand::ZenginGiveTestJournal { .. } => {
            let Some(gothic_script) = gothic_script else {
                warn!("ZenginGiveTestJournal replay command: game script is not loaded");
                return false;
            };
            let mut script = gothic_script.lock();
            let count = seed_test_journal(&mut script);
            game_log.add(format!("Seeded {count} test journal topic(s)"));
            info!("Replay seeded {count} test journal topic(s)");
        }
        ReplayCommand::OpenMainMenu { .. } => {
            info!("Replay opened the main menu");
            next_state.set(AppState::MainMenu);
        }
        ReplayCommand::ZenginSpawnPlayerCharacter {
            translation,
            rotation,
            ..
        } => {
            let transform = Transform::from_translation(*translation)
                .with_rotation(rotation_from_degrees(*rotation));
            info!("Replay requested the player character spawn at {translation}");
            commands.queue(move |world: &mut World| {
                crate::zengin::request_player_spawn(world, transform);
            });
        }
        ReplayCommand::ZenginSpawnWorld { world, .. } => {
            info!("Replay requested the zengin world spawn of {world}");
            let world_name = world.clone();
            commands.queue(move |world: &mut World| {
                crate::zengin::request_world_spawn(world, &world_name);
            });
        }
        ReplayCommand::TestWorld { name, .. } => {
            info!("Replay requested the test world {name}");
            let name = name.clone();
            commands.queue(move |world: &mut World| {
                crate::test_world::request_test_world_spawn(world, &name);
            });
        }
        ReplayCommand::ZenginFilterNpcWithName { filters, .. } => {
            npc_filter.add(filters);
            info!("Replay npc spawn name filters: {filters:?}");
        }
        ReplayCommand::ZenginFilterSpawnWithPosition { points, .. } => {
            spawn_filter.add(points);
            info!("Replay spawn position filters: {points:?}");
        }
        ReplayCommand::ZenginSetDebugNpc { name, .. } => {
            info!("Replay debug npc filter: {name:?}");
            let name = name.clone();
            commands.queue(move |world: &mut World| {
                world.resource_mut::<DebugNpc>().set(&name);
            });
        }
        ReplayCommand::ClearLogBuffer { .. } => {
            let Some(log_buffer) = log_buffer else {
                warn!("ClearLogBuffer replay command ignored: log capture is not enabled");
                return false;
            };
            log_buffer.clear();
            info!("Replay cleared the captured log buffer");
        }
        ReplayCommand::LogExists { regex, .. } => {
            let Some(log_buffer) = log_buffer else {
                warn!("LogExists replay command ignored: log capture is not enabled");
                return false;
            };
            check_log_exists(log_buffer, regex);
        }
        ReplayCommand::ExitGame { .. } => {
            if replay.reference.is_some() {
                info!("Replay requested the game exit after the screenshots are compared");
                replay.exit_requested = true;
            } else {
                info!("Replay requested the game exit");
                exit.write(AppExit::Success);
            }
        }
    }
    false
}

/// Schedules a screenshot save into the scenario workdir. Returns `true` when
/// the screenshot path is invalid and the replay must be disabled.
fn apply_replay_screenshot(replay: &ActionReplay, path: &String, commands: &mut Commands) -> bool {
    let Some(file_path) = resolve_scenario_path(&replay.workdir, path) else {
        warn!(
            "Replay disabled: screenshot path {path:?} must be relative to the \
             scenario workdir {:?}",
            replay.workdir
        );
        return true;
    };
    if let Some(parent) = file_path.parent()
        && let Err(error) = fs::create_dir_all(parent)
    {
        warn!("Failed to create screenshot dir {parent:?}: {error}");
    }
    let reference = replay.reference.as_ref().map(|root| root.join(path));
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_scenario_screenshot(
            file_path,
            reference,
            replay.update_reference,
        ));
    false
}

/// Saves the captured screenshot into the scenario workdir and, in a test
/// scenario run, compares it with the reference screenshot or, with
/// `--update-reference`, blesses it.
fn save_scenario_screenshot(
    file_path: PathBuf,
    reference: Option<PathBuf>,
    update_reference: bool,
) -> impl FnMut(On<ScreenshotCaptured>) {
    move |captured: On<ScreenshotCaptured>| {
        let Ok(image) = captured.image.clone().try_into_dynamic() else {
            panic!(
                "Failed to convert the screenshot for {} to an image",
                file_path.display()
            );
        };
        if let Err(error) = image.save_with_format(&file_path, ImageFormat::Png) {
            panic!("Failed to save screenshot {}: {error}", file_path.display());
        }
        let Some(reference) = &reference else {
            return;
        };
        if update_reference {
            bless_reference(&file_path, reference);
            return;
        }
        compare_with_reference(&image, &file_path, reference);
    }
}

/// Copies a produced screenshot over its reference screenshot.
fn bless_reference(file_path: &Path, reference: &Path) {
    if let Some(parent) = reference.parent()
        && let Err(error) = fs::create_dir_all(parent)
    {
        panic!(
            "Failed to create reference dir {}: {error}",
            parent.display()
        );
    }
    if let Err(error) = fs::copy(file_path, reference) {
        panic!(
            "Failed to copy {} to reference {}: {error}",
            file_path.display(),
            reference.display()
        );
    }
    info!("Updated reference screenshot {}", reference.display());
}

/// Compares a produced screenshot with its reference and panics when they
/// differ too much. A match only logs the difference so passing runs stay
/// quiet.
fn compare_with_reference(actual: &DynamicImage, actual_path: &Path, reference: &Path) {
    let expected = match image::open(reference) {
        Ok(image) => image,
        Err(error) => panic!(
            "Reference screenshot {} is missing or unreadable ({error}). \
             Run with --update-reference to create it",
            reference.display()
        ),
    };
    let report = compare_images(actual, &expected);
    if report.changed_fraction <= COMPARE_MAX_CHANGED_FRACTION {
        info!(
            "Screenshot {} matches its reference (changed cells {:.2}%, mean diff {:.2}/255)",
            actual_path.display(),
            report.changed_fraction * 100.0,
            report.mean_diff * 255.0
        );
        return;
    }
    let diff_path = diff_path(actual_path);
    if let Err(error) = report
        .diff_image
        .save_with_format(&diff_path, ImageFormat::Png)
    {
        warn!(
            "Failed to save screenshot difference {}: {error}",
            diff_path.display()
        );
    }
    panic!(
        "Screenshot {} differs from the reference {}: changed cells {:.1}% (threshold {:.1}%), \
         mean diff {:.1}/255. See {}",
        actual_path.display(),
        reference.display(),
        report.changed_fraction * 100.0,
        COMPARE_MAX_CHANGED_FRACTION * 100.0,
        report.mean_diff * 255.0,
        diff_path.display()
    );
}

/// Result of comparing two screenshots on the coarse comparison grid.
struct CompareReport {
    /// Fraction of grid cells whose mean channel difference exceeded the cell
    /// threshold.
    changed_fraction: f32,
    /// Mean absolute channel difference over all grid cells, `0..=1`.
    mean_diff: f32,
    /// Full resolution absolute difference, for debugging a mismatch.
    diff_image: DynamicImage,
}

/// Compares two same-sized screenshots.
///
/// Both images are downscaled to a coarse grid before the difference is
/// computed, so anti-aliasing and minor GPU noise do not count as a change,
/// while UI, geometry and lighting changes do. Panics on a size mismatch,
/// which is a large change by itself.
fn compare_images(actual: &DynamicImage, expected: &DynamicImage) -> CompareReport {
    if actual.dimensions() != expected.dimensions() {
        panic!(
            "Screenshot size {:?} does not match the reference size {:?}",
            actual.dimensions(),
            expected.dimensions()
        );
    }
    let diff_image = difference_image(actual, expected);
    let (width, height) = COMPARE_GRID;
    let actual_grid = actual
        .resize_exact(width, height, FilterType::Triangle)
        .to_rgb8();
    let expected_grid = expected
        .resize_exact(width, height, FilterType::Triangle)
        .to_rgb8();
    let mut changed = 0u32;
    let mut diff_sum = 0.0f32;
    for (actual_pixel, expected_pixel) in actual_grid.pixels().zip(expected_grid.pixels()) {
        let cell_diff = actual_pixel
            .0
            .iter()
            .zip(expected_pixel.0.iter())
            .map(|(actual, expected)| f32::from(actual.abs_diff(*expected)))
            .sum::<f32>()
            / (3.0 * 255.0);
        diff_sum += cell_diff;
        if cell_diff > COMPARE_CELL_THRESHOLD {
            changed += 1;
        }
    }
    let cells = width.saturating_mul(height);
    CompareReport {
        changed_fraction: changed as f32 / cells as f32,
        mean_diff: diff_sum / cells as f32,
        diff_image,
    }
}

/// Builds a full resolution absolute difference image.
fn difference_image(actual: &DynamicImage, expected: &DynamicImage) -> DynamicImage {
    let actual = actual.to_rgb8();
    let expected = expected.to_rgb8();
    let mut diff = actual.clone();
    for (output, (actual, expected)) in diff
        .pixels_mut()
        .zip(actual.pixels().zip(expected.pixels()))
    {
        for (output, (actual, expected)) in output
            .0
            .iter_mut()
            .zip(actual.0.iter().zip(expected.0.iter()))
        {
            *output = actual.abs_diff(*expected);
        }
    }
    DynamicImage::ImageRgb8(diff)
}

/// Difference image path next to the workdir screenshot: `screen.png` becomes
/// `screen.diff.png`.
fn diff_path(path: &Path) -> PathBuf {
    let stem = path.file_stem().map_or_else(
        || "screenshot".to_string(),
        |stem| stem.to_string_lossy().into_owned(),
    );
    path.with_file_name(format!("{stem}.diff.png"))
}

/// Exits a test scenario run after an explicit `ExitGame` once all screenshots
/// were captured and compared, so comparisons are never skipped by leaving
/// early. Plain replays exit through `ExitGame` or the frame limit.
fn finish_test_scenario(
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

/// Presses a recorded button on its input map's action state.
///
/// On the frame of the recorded press the action reports `just_pressed`;
/// on later frames of the hold it only stays `pressed`.
fn press(
    gameplay: &mut Query<&mut ActionState<GameplayAction>, With<GameplayInputMarker>>,
    gui: &mut Query<&mut ActionState<GuiAction>, With<GuiInputMarker>>,
    shared: &mut Query<&mut ActionState<SharedAction>, With<SharedInputMarker>>,
    editor: &mut Query<&mut ActionState<EditorAction>, With<EditorInputMarker>>,
    action: RecordedAction,
    started: bool,
) {
    match action {
        RecordedAction::Gameplay(action) => {
            if let Ok(mut state) = gameplay.single_mut() {
                apply_press(&mut state, &action, started);
            }
        }
        RecordedAction::Gui(action) => {
            if let Ok(mut state) = gui.single_mut() {
                apply_press(&mut state, &action, started);
            }
        }
        RecordedAction::Shared(action) => {
            if let Ok(mut state) = shared.single_mut() {
                apply_press(&mut state, &action, started);
            }
        }
        RecordedAction::Editor(action) => {
            if let Ok(mut state) = editor.single_mut() {
                apply_press(&mut state, &action, started);
            }
        }
    }
}

fn apply_press<A: Actionlike>(state: &mut ActionState<A>, action: &A, started: bool) {
    if started {
        state.press(action);
        return;
    }
    // Keep the hold pressed without retriggering `just_pressed`.
    let Some(data) = state.action_data_mut(action) else {
        return;
    };
    if let ActionKindData::Button(button) = &mut data.kind_data {
        button.state = ButtonState::Pressed;
    }
}

/// Sets a recorded dual axis value on its input map's action state.
fn set_axis_pair(
    gameplay: &mut Query<&mut ActionState<GameplayAction>, With<GameplayInputMarker>>,
    action: RecordedAction,
    value: Vec2,
) {
    let RecordedAction::Gameplay(action) = action else {
        return;
    };
    if let Ok(mut state) = gameplay.single_mut() {
        state.set_axis_pair(&action, value);
    }
}

/// Builds a quaternion from degrees labeled by axis:
/// `(x = pitch, y = yaw, z = roll)`, applied as yaw around Y first, then
/// pitch around X, then roll around Z (the standard camera convention).
fn rotation_from_degrees(rotation: Vec3) -> Quat {
    Quat::from_euler(
        EulerRot::YXZ,
        rotation.y.to_radians(),
        rotation.x.to_radians(),
        rotation.z.to_radians(),
    )
}

/// Panics when no buffered log line matches `pattern`.
fn check_log_exists(log_buffer: &LogBuffer, pattern: &str) {
    let regex = match Regex::new(pattern) {
        Ok(regex) => regex,
        Err(error) => panic!("LogExists has an invalid regex {pattern:?}: {error}"),
    };
    let lines = log_buffer.snapshot();
    if let Some(line) = matching_log_line(&lines, &regex) {
        info!("LogExists matched {pattern:?} on line: {line}");
        return;
    }
    let tail = lines
        .iter()
        .rev()
        .take(LOG_EXISTS_TAIL_LINES)
        .rev()
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    panic!(
        "LogExists did not match {pattern:?} in {} captured log line(s). Last lines:\n{tail}",
        lines.len()
    );
}

fn matching_log_line<'a>(lines: &'a [String], regex: &Regex) -> Option<&'a str> {
    lines
        .iter()
        .find(|line| regex.is_match(line))
        .map(String::as_str)
}

/// Resolves a relative path inside the scenario workdir.
///
/// Returns `None` for absolute paths, which abort the whole replay.
fn resolve_scenario_path(workdir: &Path, path: &str) -> Option<PathBuf> {
    let path = Path::new(path);
    if path.is_absolute() {
        return None;
    }
    Some(workdir.join(path))
}

/// Teleports the player entity to `translation` (model root/feet) with
/// `rotation` in axis-labeled degrees `(x = pitch, y = yaw, z = roll)`.
fn teleport_player(
    player: &mut Query<&mut Transform, (With<PlayerCharacter>, Without<Camera3d>)>,
    translation: Vec3,
    rotation: Vec3,
) {
    let Ok(mut transform) = player.single_mut() else {
        warn!("Replay teleport failed: no player character found");
        return;
    };
    transform.translation = translation;
    transform.rotation = rotation_from_degrees(rotation);
}

/// Sets the camera to `translation` with `rotation` in axis labeled degrees
/// `(x = pitch, y = yaw, z = roll)`.
fn set_camera_location(
    camera: &mut Query<&mut Transform, (With<Camera3d>, Without<PlayerCharacter>)>,
    translation: Vec3,
    rotation: Vec3,
) {
    let Ok(mut transform) = camera.single_mut() else {
        warn!("Replay set camera location failed: no camera found");
        return;
    };
    transform.translation = translation;
    transform.rotation = rotation_from_degrees(rotation);
}

/// Equips the named item on the player as a drawn weapon. Item names are
/// matched case-insensitively against the loaded item instance symbols.
fn equip_player_weapon(
    gothic_script: &ScriptResource,
    player_npc: &Query<&GameNpc, With<PlayerCharacter>>,
    item: &str,
) {
    let Ok(player_npc) = player_npc.single() else {
        warn!("Replay equip failed: no player character found");
        return;
    };
    let player_id = player_npc.instance_id();

    let mut script = gothic_script.lock();
    let lowered = item.to_lowercase();
    let Some(item_id) = script
        .get_loaded_state()
        .items_instances
        .iter()
        .find(|(_, instance)| {
            instance
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .name
                .to_lowercase()
                == lowered
        })
        .map(|(id, _)| *id)
    else {
        warn!("Replay equip failed: no item instance named {item}");
        return;
    };

    let ranged = WeaponKind::from_symbol_name(&lowered).is_some_and(|kind| kind.is_ranged());
    script.equip_item(player_id, item_id);
    script.draw_weapon(player_id, ranged);
    info!(
        "Replay equipped item({item}) on player({}) as drawn weapon",
        player_id.value()
    );
}

#[allow(clippy::cognitive_complexity)]
fn toggle_player_armor(
    gothic_script: &ScriptResource,
    player_npc: &Query<&GameNpc, With<PlayerCharacter>>,
    item: &str,
) {
    let Ok(player_npc) = player_npc.single() else {
        warn!("Replay armor toggle failed: no player character found");
        return;
    };
    let player_id = player_npc.instance_id();

    let mut script = gothic_script.lock();
    let lowered = item.to_lowercase();
    let Some(item_id) = script
        .get_loaded_state()
        .items_instances
        .iter()
        .find(|(_, instance)| {
            instance
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .name
                .to_lowercase()
                == lowered
        })
        .map(|(id, _)| *id)
    else {
        warn!("Replay armor toggle failed: no item instance named {item}");
        return;
    };

    let equipped = script.npc_state(player_id).map(|npc| npc.equipment.armor);
    if equipped == Some(Some(item_id)) {
        script.unequip_armor(player_id);
        info!(
            "Replay removed armor({item}) from player({})",
            player_id.value()
        );
    } else {
        script.equip_item(player_id, item_id);
        info!(
            "Replay equipped armor({item}) on player({})",
            player_id.value()
        );
    }
}

#[cfg(test)]
mod tests {
    use image::{DynamicImage, Rgb, RgbImage};
    use regex::Regex;
    use ron::Options as RonOptions;
    use ron::extensions::Extensions;

    use super::{
        COMPARE_MAX_CHANGED_FRACTION, ReplayCommand, ScheduledCommand, compare_images, diff_path,
        matching_log_line, schedule_commands,
    };

    fn parse(commands: &str) -> Result<Vec<ScheduledCommand>, String> {
        let commands: Vec<ReplayCommand> = RonOptions::default()
            .with_default_extension(Extensions::IMPLICIT_SOME)
            .from_str(commands)
            .expect("test ron should parse");
        schedule_commands(commands)
    }

    fn frames(commands: &[ScheduledCommand]) -> Vec<u64> {
        commands.iter().map(|command| command.frame).collect()
    }

    #[test]
    fn frame_dt_is_relative_to_previous_command() {
        let commands =
            parse("[ExitGame(frame: 10), ExitGame(frame_dt: 5), ExitGame(frame_dt: 2)]").unwrap();
        assert_eq!(frames(&commands), [10, 15, 17]);
    }

    #[test]
    fn commands_are_sorted_by_resolved_frame() {
        let commands =
            parse("[ExitGame(frame: 10), ExitGame(frame: 4), ExitGame(frame_dt: 1)]").unwrap();
        assert_eq!(frames(&commands), [4, 5, 10]);
    }

    #[test]
    fn missing_timing_defaults_to_frame_dt_5() {
        let commands = parse("[ExitGame(frame: 10), ZenginEquipWeapon(item: \"sword\")]").unwrap();
        assert_eq!(frames(&commands), [10, 15]);
    }

    #[test]
    fn command_with_both_frames_is_rejected() {
        assert!(parse("[ZenginEquipWeapon(frame: 1, frame_dt: 1, item: \"sword\")]").is_err());
    }

    #[test]
    fn first_command_frame_dt_is_relative_to_zero() {
        let commands = parse("[ZenginEquipWeapon(frame_dt: 2, item: \"sword\")]").unwrap();
        assert_eq!(frames(&commands), [2]);
    }

    #[test]
    fn bundled_scenarios_parse() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../misc/scenarios");
        let mut found = 0;
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("ron") {
                continue;
            }
            found += 1;
            parse_scenario_file(&path);
        }
        assert!(found > 0);
    }

    #[test]
    fn bundled_test_scenarios_parse() {
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../misc/test_scenarios");
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return;
        };
        for entry in entries {
            let scenario = entry.unwrap().path().join("scenario.ron");
            if scenario.exists() {
                parse_scenario_file(&scenario);
            }
        }
    }

    fn parse_scenario_file(path: &std::path::Path) {
        let contents = std::fs::read_to_string(path).unwrap();
        let commands: Vec<ReplayCommand> = RonOptions::default()
            .with_default_extension(Extensions::IMPLICIT_SOME)
            .from_str(&contents)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        schedule_commands(commands).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    }

    #[test]
    fn time_speed_command_parses_and_schedules() {
        let commands =
            parse("[ZenginSetGameTimeSpeed(frame: 4, speed: 2.5), Screenshot(path: \"shot.png\")]")
                .unwrap();
        assert_eq!(frames(&commands), [4, 9]);
        assert!(matches!(
            commands[0].command,
            ReplayCommand::ZenginSetGameTimeSpeed { speed, .. } if speed == 2.5
        ));
    }

    #[test]
    fn world_command_parses_and_schedules() {
        let commands = parse("[TestWorld(frame: 3, name: \"simple\")]").unwrap();
        assert_eq!(frames(&commands), [3]);
        assert!(matches!(
            &commands[0].command,
            ReplayCommand::TestWorld { name, .. } if name == "simple"
        ));
    }

    #[test]
    fn debug_npc_command_parses_and_schedules() {
        let commands = parse("[ZenginSetDebugNpc(frame: 3, name: \"lares\")]").unwrap();
        assert_eq!(frames(&commands), [3]);
        assert!(matches!(
            &commands[0].command,
            ReplayCommand::ZenginSetDebugNpc { name, .. } if name == "lares"
        ));
    }

    #[test]
    fn absolute_frame_serializes_without_option_wrapper() {
        let command = ReplayCommand::ExitGame {
            frame: Some(3),
            frame_dt: None,
        };
        let text = RonOptions::default()
            .with_default_extension(Extensions::IMPLICIT_SOME)
            .to_string_pretty(&vec![command], ron::ser::PrettyConfig::default())
            .unwrap();
        assert!(text.contains("frame: 3"));
        assert!(!text.contains("Some("));
        assert!(!text.contains("frame_dt"));
    }

    fn test_image(color: [u8; 3]) -> DynamicImage {
        DynamicImage::ImageRgb8(RgbImage::from_pixel(128, 96, Rgb(color)))
    }

    #[test]
    fn identical_screenshots_match() {
        let image = test_image([20, 40, 60]);
        let report = compare_images(&image, &image.clone());
        assert!(report.changed_fraction < f32::EPSILON);
        assert!(report.mean_diff < f32::EPSILON);
    }

    #[test]
    fn large_change_is_detected() {
        let black = test_image([0, 0, 0]);
        let white = test_image([255, 255, 255]);
        let report = compare_images(&black, &white);
        assert!(report.changed_fraction > COMPARE_MAX_CHANGED_FRACTION);
    }

    #[test]
    fn small_local_noise_is_accepted() {
        let image = test_image([100, 100, 100]);
        let mut noisy = image.clone().to_rgb8();
        for x in 0..12 {
            for y in 0..12 {
                noisy.put_pixel(x, y, Rgb([120, 120, 120]));
            }
        }
        let report = compare_images(&image, &DynamicImage::ImageRgb8(noisy));
        assert!(report.changed_fraction <= COMPARE_MAX_CHANGED_FRACTION);
    }

    #[test]
    fn diff_path_keeps_the_screenshot_directory() {
        assert_eq!(
            diff_path(std::path::Path::new("workdir/screens/shot.png")),
            std::path::Path::new("workdir/screens/shot.diff.png")
        );
    }

    #[test]
    fn log_commands_parse_and_schedule() {
        let commands =
            parse(r#"[ClearLogBuffer(frame: 2), LogExists(frame: 4, regex: "WARN.*missing")]"#)
                .unwrap();
        assert_eq!(frames(&commands), [2, 4]);
        assert!(matches!(
            &commands[1].command,
            ReplayCommand::LogExists { regex, .. } if regex == "WARN.*missing"
        ));
    }

    #[test]
    fn log_exists_regex_matches_buffered_lines() {
        let lines = vec![
            "INFO zengin_viewer: loaded".to_string(),
            "WARN zengin_viewer::npc: missing texture".to_string(),
        ];
        assert!(matching_log_line(&lines, &Regex::new("missing texture").unwrap()).is_some());
        assert!(matching_log_line(&lines, &Regex::new("^ERROR").unwrap()).is_none());
    }
}
