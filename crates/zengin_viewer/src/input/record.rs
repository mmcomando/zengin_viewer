//! Recording of triggered actions into a RON file, enabled by `--save-actions`.
//!
//! The recording is a frame-indexed command stream (`ReplayCommand`) that can
//! be replayed later with `--scenario` (see `input/replay.rs`).

use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;

use bevy::prelude::*;
use leafwing_input_manager::prelude::{ActionState, Actionlike, InputControlKind};
use ron::Options as RonOptions;
use ron::extensions::Extensions;
use ron::ser::PrettyConfig;
use serde::{Deserialize, Serialize};

use crate::input::actions::{
    EditorAction, EditorInputMarker, GameplayAction, GameplayInputMarker, GuiAction,
    GuiInputMarker, SharedAction, SharedInputMarker,
};
use crate::input::options::Options;
use crate::perf_debug;
use crate::zengin::{START_WORLD, SpawnPoint};

/// Third person camera switch, carrying the free camera transform the player
/// and camera are placed at. Recorded as `TeleportPlayer` and
/// `SetCameraLocation` synchronization commands and re-applied by the replay:
/// the free camera state the placement is based on is not reproducible across
/// runs.
#[derive(Message)]
pub(crate) struct CameraSwitchedToThirdPerson(pub GlobalTransform);

/// Records every triggered action together with its frame number.
pub struct ActionRecordingPlugin;

impl Plugin for ActionRecordingPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<CameraSwitchedToThirdPerson>();
        app.add_systems(Startup, open_recording_file);
        app.add_systems(Update, record_triggered_actions);
    }
}

/// A recorded action, tagged with the input map it belongs to.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum RecordedAction {
    Gameplay(GameplayAction),
    Gui(GuiAction),
    Shared(SharedAction),
    Editor(EditorAction),
}

/// sRGB color components in the `0..=1` range, the `PlaceCube` command's
/// `color` field.
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReplayColor {
    pub(crate) red: f32,
    pub(crate) green: f32,
    pub(crate) blue: f32,
}

/// One recorded replay command. Together these fully describe the triggered
/// inputs and extra commands of a run so they can be replayed frame by frame.
///
/// Every command runs [`DEFAULT_FRAME_DT`] frames after the previous one;
/// [`Self::Wait`] instead delays the commands that follow it.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub(crate) enum ReplayCommand {
    /// Delays the following commands by `frame_dt` frames without applying
    /// anything.
    Wait { frame_dt: u64 },
    /// Button was pressed; replay holds it until a matching `Release`.
    Press { action: RecordedAction },
    /// Button was released.
    Release { action: RecordedAction },
    /// Button was pressed and released again before the next frame; it
    /// reports `just_pressed` on this frame without staying held.
    PressRelease { action: RecordedAction },
    /// Dual axis value changed to `value`.
    Axis { action: RecordedAction, value: Vec2 },
    /// Teleport the player character to `translation` with `rotation` in axis
    /// labeled degrees `(x = pitch, y = yaw, z = roll)`, matching the camera
    /// info overlay.
    TeleportPlayer { translation: Vec3, rotation: Vec3 },
    /// Set the camera location to `translation` with `rotation` in axis labeled
    /// degrees `(x = pitch, y = yaw, z = roll)`, matching the camera info
    /// overlay.
    SetCameraLocation { translation: Vec3, rotation: Vec3 },
    /// Equips the item with the given (case-insensitive) instance symbol name
    /// on the player as a drawn weapon.
    ZenginEquipWeapon { item: String },
    /// Sets the player's 1H weapon talent skill (`1` trained, `2` master).
    ZenginSetPlayerOneHandSkill { skill: i32 },
    /// Sets the player's 2H weapon talent skill (`1` trained, `2` master).
    ZenginSetPlayerTwoHandSkill { skill: i32 },
    /// Sets the player's `C_Npc` attribute by name (`strength`, `dexterity`).
    ZenginSetPlayerAttribute { attribute: String, value: i32 },
    /// Sets the player's `C_Npc.hitchance[talent]`, the melee critical roll.
    ZenginSetPlayerFightSkill { talent: i32, value: i32 },
    /// Sets the in-game time speed multiplier, `0.0` pauses the time. Replays
    /// start with a multiplier of `0.0`.
    ZenginSetGameTimeSpeed { speed: f32 },
    /// Moves the in-game clock to the next occurrence of `hour:minute`, matching
    /// `Wld_SetTime`.
    ZenginSetGameTime { hour: i32, minute: i32 },
    /// Toggles the armor item with the given (case-insensitive) instance symbol
    /// name on the player.
    ZenginToggleArmor { item: String },
    /// Save a screenshot of the primary window to `path`.
    Screenshot { path: String },
    /// Save a screenshot of the primary window to `path` and compare it with
    /// the reference screenshot at the same relative path under
    /// `misc/scenario_test_references/<scenario id>/`. With `--update-reference`
    /// the produced screenshot is copied into the reference directory instead.
    TestScreenshot { path: String },
    /// Grants the hardcoded test items to the player.
    ZenginGiveTestItems,
    /// Seeds the hardcoded test journal topics.
    ZenginGiveTestJournal,
    /// Switches to the main menu.
    OpenMainMenu,
    /// Spawns the player character at `translation` with `rotation` in axis
    /// labeled degrees `(x = pitch, y = yaw, z = roll)`, matching the camera
    /// info overlay.
    ZenginSpawnPlayerCharacter { translation: Vec3, rotation: Vec3 },
    /// Spawns the zengin world `world` (path relative to the worlds directory
    /// without the `.ZEN` extension). Replays load no world unless they
    /// request it with this command.
    ZenginSpawnWorld { world: String },
    /// Runs the world script entry points of `world` without loading its
    /// zengin world. Replay runs skip the default world spawn, so a test world
    /// scenario needs this to get the game state a new game sets up (e.g. the
    /// `KAPITEL` chapter used by dialog conditions). `world` uses the same
    /// format as [`Self::ZenginSpawnWorld`].
    ///
    /// `clean_world` also runs the `startup_*` entry points, which assume an
    /// empty world and insert its npcs/items. Only set it when the scenario
    /// loads the matching zengin world too.
    ZenginScriptStartup {
        world: String,
        #[serde(default)]
        clean_world: bool,
    },
    /// Adds name filters for npc spawning. An npc is spawned when its script
    /// symbol name or display name contains any of `filters`
    /// (case-insensitive). Repeated commands extend the same list.
    ZenginFilterNpcWithName { filters: Vec<String> },
    /// Restricts spawning to the union of the given circles: an entity is
    /// spawned only when its position is within `radius` world units of a
    /// point. Repeated commands extend the same list. The world mesh,
    /// waypoint navigation nodes and the player are not filtered.
    ZenginFilterSpawnWithPosition { points: Vec<SpawnPoint> },
    /// Adds the npc traced by the debug npc trace, matched as a
    /// case-insensitive substring of its script symbol name. Repeated commands
    /// extend the trace; `clear` drops the npcs traced before.
    ZenginSetDebugNpc {
        name: String,
        #[serde(default)]
        clear: bool,
    },
    /// Kills the npc instance `name` through `Npc_ChangeAttribute`, exercising
    /// the script driven death flow.
    ZenginKillNpc { name: String },
    /// Spawns the npc instance `name` (case-insensitive script symbol name) at
    /// `translation` with `rotation` in axis labeled degrees
    /// `(x = pitch, y = yaw, z = roll)`, matching the camera info overlay.
    ZenginSpawnNpc {
        name: String,
        translation: Vec3,
        rotation: Vec3,
    },
    /// Adds a waypoint node `name` at `translation` to the navigation graph, so
    /// scenarios without a world way net (test worlds) can walk npcs. When
    /// `connect_to` names an existing waypoint, the two nodes are joined by an
    /// edge; isolated nodes cannot be reached by path finding.
    ZenginAddWaypoint {
        name: String,
        translation: Vec3,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        connect_to: Option<String>,
    },
    /// Makes the npc instance `name` (case-insensitive script symbol name) walk
    /// to the waypoint `waypoint` (case-insensitive) by queueing a `GoToWaypoint`
    /// action, bypassing the daily routine. The waypoint must exist in the way
    /// net or have been added with [`Self::ZenginAddWaypoint`].
    ZenginNpcGotoWaypoint { name: String, waypoint: String },
    /// Makes the npc instance `name` (case-insensitive script symbol name) use
    /// the nearest free mob of the `AI_UseMob` scheme `scheme`, bypassing the
    /// daily routine. `state <= 0` releases the current mob instead.
    ZenginNpcUseMob {
        name: String,
        scheme: String,
        state: i32,
    },
    /// Spawns the interactive mob (oCMobInter) using the visual mesh `visual`
    /// at `translation` with `rotation` in axis labeled degrees
    /// `(x = pitch, y = yaw, z = roll)`. The visual is a mesh name like
    /// `CHAIR_NW_NORMAL_01` resolved against the game archives, so no world
    /// load is needed. `focus_name` defaults to the mob scheme, `state_num` to
    /// one interaction state. `on_state_func` is the script hook the mob would
    /// carry on a real map (`sleepabit` for beds); `<name>_S<state>` runs when
    /// the player or an npc reaches the state.
    ZenginSpawnMob {
        visual: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        focus_name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        state_num: Option<i32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        on_state_func: Option<String>,
        translation: Vec3,
        rotation: Vec3,
    },
    /// Spawns an empty `oCMobContainer` using the visual mesh `visual` at
    /// `translation` with `rotation` in axis labeled degrees. `key` and
    /// `pick_string` set the lock requirements. Used by the chest animation
    /// scenario.
    ZenginSpawnChest {
        visual: String,
        #[serde(default)]
        locked: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        key: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pick_string: Option<String>,
        translation: Vec3,
        rotation: Vec3,
    },
    /// Spawns a static, colliding cube at `translation` with `size` in world
    /// units and `rotation` in axis labeled degrees. Adds geometry the test
    /// worlds do not provide, e.g. a wall between two actors.
    PlaceCube {
        translation: Vec3,
        rotation: Vec3,
        size: Vec3,
        color: ReplayColor,
    },
    /// Spawns the test world `name` defined in code, with its own camera.
    /// Does not load a zengin world archive.
    TestWorld { name: String },
    /// Includes the commands of another scenario at this point. `path` is
    /// absolute in the scenario namespace, so `/test_room` names
    /// `<scenario root>/test_room.ron`; it cannot point outside the root.
    IncludeScenario { path: String },
    /// Exits the application.
    ExitGame,
    /// Panics when no captured log line matches the regex.
    TestLogExists { regex: String },
    /// Panics when a captured log line matches the regex, the negative of
    /// [`Self::TestLogExists`]. Usually paired with [`Self::TestClearLogBuffer`]
    /// so it only checks the events of the observed time window.
    TestLogNotExists { regex: String },
    /// Clears the captured log lines.
    TestClearLogBuffer,
}

/// Frames a command runs after the previous one unless it is a
/// [`ReplayCommand::Wait`].
pub(crate) const DEFAULT_FRAME_DT: u64 = 5;

/// Recorded commands plus the frame the next appended command will run at.
struct RecordedCommands {
    commands: Vec<ReplayCommand>,
    next_frame: u64,
}

impl Default for RecordedCommands {
    fn default() -> Self {
        Self {
            commands: Vec::new(),
            next_frame: DEFAULT_FRAME_DT,
        }
    }
}

impl RecordedCommands {
    /// Appends `command` so that it runs on `frame`, inserting a [`ReplayCommand::Wait`]
    /// when the default gap to the previous command is too short. Commands that
    /// share a frame are spread `DEFAULT_FRAME_DT` apart instead.
    fn push(&mut self, frame: u64, command: ReplayCommand) {
        let target = frame.max(self.next_frame);
        let wait = target - self.next_frame;
        if wait > 0 {
            self.commands.push(ReplayCommand::Wait { frame_dt: wait });
        }
        self.commands.push(command);
        self.next_frame = target + DEFAULT_FRAME_DT;
    }
}

/// In-memory recording, rewritten to the target file whenever new commands
/// appear.
#[derive(Resource)]
struct ActionRecording {
    path: PathBuf,
    frame: u64,
    commands: RecordedCommands,
    /// Buttons currently recorded as pressed, to detect releases even when
    /// the action gets disabled before it is released.
    held_gameplay: HashSet<GameplayAction>,
    held_gui: HashSet<GuiAction>,
    held_shared: HashSet<SharedAction>,
    held_editor: HashSet<EditorAction>,
    /// Last recorded dual axis values, to detect changes.
    last_axis: HashMap<RecordedAction, Vec2>,
}

/// Removes any previous recording file.
fn open_recording_file(options: Res<Options>, mut commands: Commands) {
    let _perf = perf_debug!();
    let Some(path) = options.save_actions() else {
        return;
    };
    if let Some(parent) = path.parent()
        && let Err(error) = fs::create_dir_all(parent)
    {
        warn!("Failed to create action recording dir {parent:?}: {error}");
    }
    if let Err(error) = fs::remove_file(path)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        warn!("Failed to remove old action recording {path:?}: {error}");
    }
    commands.insert_resource(ActionRecording {
        path: path.clone(),
        frame: 0,
        // Recorded scenarios need an explicit world spawn to replay in a world.
        commands: {
            let mut commands = RecordedCommands::default();
            commands.push(
                0,
                ReplayCommand::ZenginSpawnWorld {
                    world: START_WORLD.to_string(),
                },
            );
            commands
        },
        held_gameplay: HashSet::new(),
        held_gui: HashSet::new(),
        held_shared: HashSet::new(),
        held_editor: HashSet::new(),
        last_axis: HashMap::new(),
    });
}

/// Collects input events of the current frame and saves the recording file.
fn record_triggered_actions(
    mut recording: Option<ResMut<ActionRecording>>,
    gameplay: Option<Single<&ActionState<GameplayAction>, With<GameplayInputMarker>>>,
    gui: Option<Single<&ActionState<GuiAction>, With<GuiInputMarker>>>,
    shared: Option<Single<&ActionState<SharedAction>, With<SharedInputMarker>>>,
    editor: Option<Single<&ActionState<EditorAction>, With<EditorInputMarker>>>,
    mut camera_switches: MessageReader<CameraSwitchedToThirdPerson>,
) {
    let _perf = perf_debug!();
    let Some(recording) = recording.as_deref_mut() else {
        return;
    };
    recording.frame += 1;
    let frame = recording.frame;

    let before = recording.commands.commands.len();

    let gameplay = gameplay.map(|state| state.into_inner());
    let gui = gui.map(|state| state.into_inner());
    let shared = shared.map(|state| state.into_inner());
    let editor = editor.map(|state| state.into_inner());

    // Commands are collected in a local vector to avoid double borrows of the
    // resource through the separate held-state fields.
    let mut commands = std::mem::take(&mut recording.commands);

    record_buttons(
        gameplay,
        RecordedAction::Gameplay,
        frame,
        &mut commands,
        &mut recording.held_gameplay,
    );
    record_buttons(
        gui,
        RecordedAction::Gui,
        frame,
        &mut commands,
        &mut recording.held_gui,
    );
    record_buttons(
        shared,
        RecordedAction::Shared,
        frame,
        &mut commands,
        &mut recording.held_shared,
    );
    record_buttons(
        editor,
        RecordedAction::Editor,
        frame,
        &mut commands,
        &mut recording.held_editor,
    );

    // The action enums are iterable, so all dual axis actions of every map
    // are recorded; new axis actions are picked up automatically.
    record_map_axes(
        gameplay,
        RecordedAction::Gameplay,
        frame,
        &mut commands,
        &mut recording.last_axis,
    );
    record_map_axes(
        gui,
        RecordedAction::Gui,
        frame,
        &mut commands,
        &mut recording.last_axis,
    );
    record_map_axes(
        shared,
        RecordedAction::Shared,
        frame,
        &mut commands,
        &mut recording.last_axis,
    );
    record_map_axes(
        editor,
        RecordedAction::Editor,
        frame,
        &mut commands,
        &mut recording.last_axis,
    );

    // Extra commands of this frame, written from `PreUpdate` systems.
    for switch in camera_switches.read() {
        let (translation, player_rotation, camera_rotation) = switch_sync_transforms(&switch.0);
        commands.push(
            frame,
            ReplayCommand::TeleportPlayer {
                translation,
                rotation: player_rotation,
            },
        );
        commands.push(
            frame,
            ReplayCommand::SetCameraLocation {
                translation,
                rotation: camera_rotation,
            },
        );
    }

    recording.commands = commands;
    if recording.commands.commands.len() != before {
        save_recording(recording);
    }
}

/// Records button presses and releases of one input map.
///
/// Presses come from the action state; releases are detected by watching
/// `pressed()`, which also works when the action is disabled mid-hold (the
/// action state reports it as not pressed without emitting a release event).
fn record_buttons<A: Actionlike + Copy>(
    state: Option<&ActionState<A>>,
    to_recorded: impl Fn(A) -> RecordedAction,
    frame: u64,
    commands: &mut RecordedCommands,
    held: &mut HashSet<A>,
) {
    let Some(state) = state else {
        return;
    };
    for action in state.get_just_pressed() {
        if !held.insert(action) {
            continue;
        }
        if state.pressed(&action) {
            commands.push(
                frame,
                ReplayCommand::Press {
                    action: to_recorded(action),
                },
            );
            continue;
        }
        // Released again before the next frame, a separate `Release` would
        // cancel the press before the replay ever applies it.
        held.remove(&action);
        commands.push(
            frame,
            ReplayCommand::PressRelease {
                action: to_recorded(action),
            },
        );
    }
    held.retain(|action| {
        if state.pressed(action) {
            return true;
        }
        commands.push(
            frame,
            ReplayCommand::Release {
                action: to_recorded(*action),
            },
        );
        false
    });
}

/// Records dual axis value changes of one input map.
///
/// Iterates over all variants of the action enum, so newly added axis actions
/// are recorded without changes here; the control kind decides what is
/// recorded.
fn record_map_axes<A: Actionlike + Copy + strum::IntoEnumIterator>(
    state: Option<&ActionState<A>>,
    to_recorded: impl Fn(A) -> RecordedAction,
    frame: u64,
    commands: &mut RecordedCommands,
    last_axis: &mut HashMap<RecordedAction, Vec2>,
) {
    let Some(state) = state else {
        return;
    };
    for action in A::iter() {
        if action.input_control_kind() != InputControlKind::DualAxis {
            continue;
        }
        record_axis(
            to_recorded(action),
            state.axis_pair(&action),
            frame,
            commands,
            last_axis,
        );
    }
}

/// Records a dual axis value change, including the return to zero.
fn record_axis(
    action: RecordedAction,
    value: Vec2,
    frame: u64,
    commands: &mut RecordedCommands,
    last_axis: &mut HashMap<RecordedAction, Vec2>,
) {
    let unchanged = last_axis
        .get(&action)
        .is_some_and(|last| last.distance(value) < 0.001);
    if unchanged {
        return;
    }
    last_axis.insert(action, value);
    commands.push(frame, ReplayCommand::Axis { action, value });
}

/// Player teleport and camera location of a camera switch. The player is
/// placed at the camera position facing its horizontal look direction, the
/// camera keeps its full rotation. Rotations are axis labeled degrees
/// `(x = pitch, y = yaw, z = roll)`.
/// Returns `(translation, player_rotation, camera_rotation)`.
pub(crate) fn switch_sync_transforms(camera: &GlobalTransform) -> (Vec3, Vec3, Vec3) {
    let translation = camera.translation();
    let (yaw, pitch, roll) = camera.rotation().to_euler(EulerRot::YXZ);
    let camera_rotation = Vec3::new(pitch.to_degrees(), yaw.to_degrees(), roll.to_degrees());
    let forward = camera.forward();
    let dir = Vec3::new(forward.x, 0.0, forward.z);
    if dir.length_squared() <= f32::EPSILON {
        warn!(
            "Camera switch looks straight up or down ({camera_rotation:?}), \
             recording the player facing north"
        );
        return (translation, Vec3::ZERO, camera_rotation);
    }
    let dir = dir.normalize();
    let player_rotation = Vec3::new(0.0, dir.x.atan2(dir.z).to_degrees(), 0.0);
    (translation, player_rotation, camera_rotation)
}

/// Rewrites the whole recording file with the current command list.
fn save_recording(recording: &ActionRecording) {
    let mut pretty_config = PrettyConfig::default();
    pretty_config.depth_limit = 1;
    let contents = match RonOptions::default()
        .with_default_extension(Extensions::IMPLICIT_SOME)
        .to_string_pretty(&recording.commands.commands, pretty_config)
    {
        Ok(contents) => contents,
        Err(error) => {
            warn!("Failed to serialize action recording: {error}");
            return;
        }
    };
    let result = File::create(&recording.path).and_then(|mut file| {
        writeln!(
            file,
            "// Recorded replay commands, replayable with --scenario <id>"
        )?;
        writeln!(file, "{contents}")
    });
    if let Err(error) = result {
        warn!(
            "Failed to write action recording file {:?}: {error}",
            recording.path
        );
    }
}
