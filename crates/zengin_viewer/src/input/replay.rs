//! Replay of recorded commands from a RON file, enabled by `--replay-actions`.
//!
//! The file can live anywhere; paths inside it (screenshots) are resolved
//! relative to `misc/scenario_output/<file stem>/` and must not be absolute.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy::render::view::window::screenshot::{Screenshot, save_to_disk};
use leafwing_input_manager::action_state::ActionKindData;
use leafwing_input_manager::buttonlike::ButtonState;
use leafwing_input_manager::plugin::InputManagerSystem;
use leafwing_input_manager::prelude::{ActionState, Actionlike};
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
use crate::input::options::Options;
use crate::input::record::{DEFAULT_FRAME_DT, RecordedAction, ReplayCommand};
use crate::zengin::{
    GameNpc, NpcSpawnNameFilter, ScriptResource, SpawnPositionFilter, WeaponKind, grant_test_items,
    seed_test_journal,
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
        app.init_resource::<PendingLocations>();
    }
}

/// Root directory of all scenario workdirs.
const SCENARIO_DIR: &str = "misc/scenario_output";

/// Resolution used for scenario runs.
const SCENARIO_WINDOW_RESOLUTION: (u32, u32) = (1024, 768);

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

/// Replay schedule loaded from the scenario file plus the replay state.
#[derive(Resource)]
struct ActionReplay {
    /// Workdir used to resolve relative paths inside the commands.
    workdir: PathBuf,
    /// Commands with resolved frames, sorted by frame.
    commands: Vec<ScheduledCommand>,
    /// Index of the next event to apply.
    next: usize,
    frame: u64,
    /// Buttons currently held by the replay.
    held_buttons: HashSet<RecordedAction>,
    /// Buttons whose press event was processed this frame; these get a real
    /// `JustPressed` press, later held frames only keep the pressed state.
    started_buttons: HashSet<RecordedAction>,
    /// Latest dual axis values applied by the replay.
    held_axes: HashMap<RecordedAction, Vec2>,
}

/// Loads and prepares the replay file.
///
/// Outputs of the replay (screenshots) resolve against the scenario workdir
/// named after the replay file.
fn load_replay_file(options: Res<Options>, mut commands: Commands) {
    let Some(path) = options.replay_actions() else {
        return;
    };
    let Some(replay) = prepare_replay(path) else {
        return;
    };
    info!(
        "Replaying {} recorded replay command(s) from {path:?}",
        replay.commands.len()
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

/// Resets the scenario workdir and parses the replay file. Returns `None`
/// (with a warning logged) when the workdir cannot be prepared or the replay
/// file is unreadable, disabling the replay.
///
/// Panics when the scenario file itself does not exist, since that is a broken
/// invocation rather than a degraded replay.
fn prepare_replay(path: &Path) -> Option<ActionReplay> {
    if !path.exists() {
        panic!("Scenario file {} does not exist", path.display());
    }
    let workdir = scenario_workdir(path);
    reset_scenario_workdir(&workdir);
    match read_replay(&workdir, path) {
        Ok(replay) => Some(replay),
        Err(error) => {
            warn!("Replaying actions disabled: {error}");
            None
        }
    }
}

fn read_replay(workdir: &Path, path: &Path) -> Result<ActionReplay, String> {
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let commands: Vec<ReplayCommand> = RonOptions::default()
        .with_default_extension(Extensions::IMPLICIT_SOME)
        .from_str(&contents)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))?;
    let commands = schedule_commands(commands)?;
    Ok(ActionReplay {
        workdir: workdir.to_path_buf(),
        commands,
        next: 0,
        frame: 0,
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
        ReplayCommand::SetGameTimeSpeed { speed, .. } => {
            game_time.speed = *speed;
            info!("Replay set the in-game time speed multiplier to {speed}");
        }
        ReplayCommand::EquipWeapon { item, .. } => {
            let Some(gothic_script) = gothic_script else {
                warn!("EquipWeapon replay command: game script is not loaded");
                return false;
            };
            equip_player_weapon(gothic_script, player_npc, item);
        }
        ReplayCommand::ToggleArmor { item, .. } => {
            let Some(gothic_script) = gothic_script else {
                warn!("ToggleArmor replay command: game script is not loaded");
                return false;
            };
            toggle_player_armor(gothic_script, player_npc, item);
        }
        ReplayCommand::Screenshot { path, .. } => {
            return apply_replay_screenshot(replay, path, commands);
        }
        ReplayCommand::GiveTestItems { .. } => {
            let Some(gothic_script) = gothic_script else {
                warn!("GiveTestItems replay command: game script is not loaded");
                return false;
            };
            let Ok(player_npc) = player_npc.single() else {
                warn!("GiveTestItems replay command: player npc is not spawned");
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
        ReplayCommand::GiveTestJournal { .. } => {
            let Some(gothic_script) = gothic_script else {
                warn!("GiveTestJournal replay command: game script is not loaded");
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
        ReplayCommand::SpawnWorld { world, .. } => {
            info!("Replay requested the world spawn of {world}");
            let world_name = world.clone();
            commands.queue(move |world: &mut World| {
                crate::zengin::request_world_spawn(world, &world_name);
            });
        }
        ReplayCommand::FilterNpcWithName { filters, .. } => {
            npc_filter.add(filters);
            info!("Replay npc spawn name filters: {filters:?}");
        }
        ReplayCommand::FilterSpawnWithPosition { points, .. } => {
            spawn_filter.add(points);
            info!("Replay spawn position filters: {points:?}");
        }
        ReplayCommand::ExitGame { .. } => {
            info!("Replay requested the game exit");
            exit.write(AppExit::Success);
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
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(file_path.to_string_lossy().into_owned()));
    false
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
    use ron::Options as RonOptions;
    use ron::extensions::Extensions;

    use super::{ReplayCommand, ScheduledCommand, schedule_commands};

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
        let commands = parse("[ExitGame(frame: 10), EquipWeapon(item: \"sword\")]").unwrap();
        assert_eq!(frames(&commands), [10, 15]);
    }

    #[test]
    fn command_with_both_frames_is_rejected() {
        assert!(parse("[EquipWeapon(frame: 1, frame_dt: 1, item: \"sword\")]").is_err());
    }

    #[test]
    fn first_command_frame_dt_is_relative_to_zero() {
        let commands = parse("[EquipWeapon(frame_dt: 2, item: \"sword\")]").unwrap();
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
            let contents = std::fs::read_to_string(&path).unwrap();
            let commands: Vec<ReplayCommand> = RonOptions::default()
                .with_default_extension(Extensions::IMPLICIT_SOME)
                .from_str(&contents)
                .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            schedule_commands(commands)
                .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        }
        assert!(found > 0);
    }

    #[test]
    fn time_speed_command_parses_and_schedules() {
        let commands =
            parse("[SetGameTimeSpeed(frame: 4, speed: 2.5), Screenshot(path: \"shot.png\")]")
                .unwrap();
        assert_eq!(frames(&commands), [4, 9]);
        assert!(matches!(
            commands[0].command,
            ReplayCommand::SetGameTimeSpeed { speed, .. } if speed == 2.5
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
}
