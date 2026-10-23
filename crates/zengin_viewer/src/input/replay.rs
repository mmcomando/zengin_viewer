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

use crate::gameplay::character::PlayerPhysicsBodyMarker;
use crate::gameplay::character_camera::camera_toggle_input;
use crate::gameplay::objects::PlayerCharacter;
use crate::input::actions::{
    DebugAction, DebugInputMarker, GameplayAction, GameplayInputMarker, GuiAction, GuiInputMarker,
    SharedAction, SharedInputMarker, update_input_mode,
};
use crate::input::options::Options;
use crate::input::record::{RecordedAction, ReplayCommand};
use crate::zengin::{GameNpc, ScriptResource, WeaponKind};

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
    let name = replay_path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "replay".to_string());
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

/// Replay schedule loaded from the scenario file plus the replay state.
#[derive(Resource)]
struct ActionReplay {
    /// Workdir used to resolve relative paths inside the commands.
    workdir: PathBuf,
    /// Commands sorted by frame.
    commands: Vec<ReplayCommand>,
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
    let workdir = scenario_workdir(path);
    // Start from an empty workdir so everything left in it was produced by
    // this run, not by an earlier one.
    match fs::remove_dir_all(&workdir) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => warn!("Failed to remove old scenario workdir {workdir:?}: {error}"),
    }
    if let Err(error) = fs::create_dir_all(&workdir) {
        warn!("Failed to create scenario workdir {workdir:?}: {error}");
    }
    let replay = match read_replay(&workdir, path) {
        Ok(replay) => replay,
        Err(error) => {
            warn!("Replaying actions disabled: {error}");
            return;
        }
    };
    info!(
        "Replaying {} recorded replay command(s) from {path:?}",
        replay.commands.len()
    );
    commands.insert_resource(replay);
}

fn read_replay(workdir: &Path, path: &Path) -> Result<ActionReplay, String> {
    let contents =
        fs::read_to_string(path).map_err(|error| format!("failed to read {path:?}: {error}"))?;
    let mut commands: Vec<ReplayCommand> = ron::de::from_str(&contents)
        .map_err(|error| format!("failed to parse {path:?}: {error}"))?;
    commands.sort_by_key(|command| command.frame());
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
    mut player: Query<&mut Transform, (With<PlayerPhysicsBodyMarker>, Without<Camera3d>)>,
    mut player_visual: Query<
        &mut Transform,
        (
            With<PlayerCharacter>,
            Without<PlayerPhysicsBodyMarker>,
            Without<Camera3d>,
        ),
    >,
    mut camera: Query<&mut Transform, (With<Camera3d>, Without<PlayerPhysicsBodyMarker>)>,
) {
    for (translation, rotation) in pending.player.drain(..) {
        teleport_player(&mut player, &mut player_visual, translation, rotation);
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
    mut debug: Query<&mut ActionState<DebugAction>, With<DebugInputMarker>>,
    gothic_script: Res<ScriptResource>,
    player_npc: Query<&GameNpc, With<PlayerCharacter>>,
    mut pending: ResMut<PendingLocations>,
) {
    let Some(mut replay) = replay else {
        return;
    };
    replay.frame += 1;
    replay.started_buttons.clear();

    while replay.next < replay.commands.len()
        && replay.commands[replay.next].frame() <= replay.frame
    {
        let command = replay.commands[replay.next].clone();
        replay.next += 1;
        info!("Replay triggered command: {command:?}");
        match &command {
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
            ReplayCommand::EquipWeapon { item, .. } => {
                equip_player_weapon(&gothic_script, &player_npc, item);
            }
            ReplayCommand::Screenshot { path, .. } => {
                let Some(file_path) = resolve_scenario_path(&replay.workdir, path) else {
                    warn!(
                        "Replay disabled: screenshot path {path:?} must be relative to the \
                         scenario workdir {:?}",
                        replay.workdir
                    );
                    commands.remove_resource::<ActionReplay>();
                    return;
                };
                if let Some(parent) = file_path.parent() {
                    if let Err(error) = fs::create_dir_all(parent) {
                        warn!("Failed to create screenshot dir {parent:?}: {error}");
                    }
                }
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk(file_path.to_string_lossy().into_owned()));
            }
        }
    }

    for action in replay.held_buttons.iter() {
        let started = replay.started_buttons.contains(action);
        press(
            &mut gameplay,
            &mut gui,
            &mut shared,
            &mut debug,
            *action,
            started,
        );
    }
    for (action, value) in replay.held_axes.iter() {
        set_axis_pair(&mut gameplay, *action, *value);
    }
}

/// Presses a recorded button on its input map's action state.
///
/// On the frame of the recorded press the action reports `just_pressed`;
/// on later frames of the hold it only stays `pressed`.
fn press(
    gameplay: &mut Query<&mut ActionState<GameplayAction>, With<GameplayInputMarker>>,
    gui: &mut Query<&mut ActionState<GuiAction>, With<GuiInputMarker>>,
    shared: &mut Query<&mut ActionState<SharedAction>, With<SharedInputMarker>>,
    debug: &mut Query<&mut ActionState<DebugAction>, With<DebugInputMarker>>,
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
        RecordedAction::Debug(action) => {
            if let Ok(mut state) = debug.single_mut() {
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

/// Teleports the player character to `translation` with `rotation` in axis
/// labeled degrees `(x = pitch, y = yaw, z = roll)`. The rotation is applied
/// to the visual model: the facing lives there, the physics body rotation
/// stays identity.
fn teleport_player(
    player: &mut Query<&mut Transform, (With<PlayerPhysicsBodyMarker>, Without<Camera3d>)>,
    player_visual: &mut Query<
        &mut Transform,
        (
            With<PlayerCharacter>,
            Without<PlayerPhysicsBodyMarker>,
            Without<Camera3d>,
        ),
    >,
    translation: Vec3,
    rotation: Vec3,
) {
    let Ok(mut transform) = player.single_mut() else {
        warn!("Replay teleport failed: no player character found");
        return;
    };
    transform.translation = translation;
    let Ok(mut visual_transform) = player_visual.single_mut() else {
        warn!("Replay teleport failed: no player visual found");
        return;
    };
    visual_transform.rotation = rotation_from_degrees(rotation);
}

/// Sets the camera to `translation` with `rotation` in axis labeled degrees
/// `(x = pitch, y = yaw, z = roll)`.
fn set_camera_location(
    camera: &mut Query<&mut Transform, (With<Camera3d>, Without<PlayerPhysicsBodyMarker>)>,
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
        .find(|(_, instance)| instance.name.to_lowercase() == lowered)
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
