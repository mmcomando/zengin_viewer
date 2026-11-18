//! Recording of triggered actions into a RON file, enabled by `--save-actions`.
//!
//! The recording is a frame-indexed command stream (`ReplayCommand`) that can
//! be replayed later with `--replay-actions` (see `input/replay.rs`).

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

/// One recorded replay command. Together these fully describe the triggered
/// inputs and extra commands of a run so they can be replayed frame by frame.
///
/// A command runs either on an absolute `frame` or a `frame_dt` after the
/// previous command's frame. Missing both means `frame_dt` of
/// [`DEFAULT_FRAME_DT`].
#[derive(Serialize, Deserialize, Clone, Debug)]
pub(crate) enum ReplayCommand {
    /// Button was pressed; replay holds it until a matching `Release`.
    Press {
        #[serde(skip_serializing_if = "Option::is_none")]
        frame: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        frame_dt: Option<u64>,
        action: RecordedAction,
    },
    /// Button was released.
    Release {
        #[serde(skip_serializing_if = "Option::is_none")]
        frame: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        frame_dt: Option<u64>,
        action: RecordedAction,
    },
    /// Dual axis value changed to `value`.
    Axis {
        #[serde(skip_serializing_if = "Option::is_none")]
        frame: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        frame_dt: Option<u64>,
        action: RecordedAction,
        value: Vec2,
    },
    /// Teleport the player character to `translation` with `rotation` in axis
    /// labeled degrees `(x = pitch, y = yaw, z = roll)`, matching the camera
    /// info overlay.
    TeleportPlayer {
        #[serde(skip_serializing_if = "Option::is_none")]
        frame: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        frame_dt: Option<u64>,
        translation: Vec3,
        rotation: Vec3,
    },
    /// Set the camera location to `translation` with `rotation` in axis labeled
    /// degrees `(x = pitch, y = yaw, z = roll)`, matching the camera info
    /// overlay.
    SetCameraLocation {
        #[serde(skip_serializing_if = "Option::is_none")]
        frame: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        frame_dt: Option<u64>,
        translation: Vec3,
        rotation: Vec3,
    },
    /// Equips the item with the given (case-insensitive) instance symbol name
    /// on the player as a drawn weapon.
    ZenginEquipWeapon {
        #[serde(skip_serializing_if = "Option::is_none")]
        frame: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        frame_dt: Option<u64>,
        item: String,
    },
    /// Sets the in-game time speed multiplier, `0.0` pauses the time. Replays
    /// start with a multiplier of `0.0`.
    ZenginSetGameTimeSpeed {
        #[serde(skip_serializing_if = "Option::is_none")]
        frame: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        frame_dt: Option<u64>,
        speed: f32,
    },
    /// Toggles the armor item with the given (case-insensitive) instance symbol
    /// name on the player.
    ZenginToggleArmor {
        #[serde(skip_serializing_if = "Option::is_none")]
        frame: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        frame_dt: Option<u64>,
        item: String,
    },
    /// Save a screenshot of the primary window to `path`.
    Screenshot {
        #[serde(skip_serializing_if = "Option::is_none")]
        frame: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        frame_dt: Option<u64>,
        path: String,
    },
    /// Grants the hardcoded test items to the player.
    ZenginGiveTestItems {
        #[serde(skip_serializing_if = "Option::is_none")]
        frame: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        frame_dt: Option<u64>,
    },
    /// Seeds the hardcoded test journal topics.
    ZenginGiveTestJournal {
        #[serde(skip_serializing_if = "Option::is_none")]
        frame: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        frame_dt: Option<u64>,
    },
    /// Switches to the main menu.
    OpenMainMenu {
        #[serde(skip_serializing_if = "Option::is_none")]
        frame: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        frame_dt: Option<u64>,
    },
    /// Spawns the player character at `translation` with `rotation` in axis
    /// labeled degrees `(x = pitch, y = yaw, z = roll)`, matching the camera
    /// info overlay.
    ZenginSpawnPlayerCharacter {
        #[serde(skip_serializing_if = "Option::is_none")]
        frame: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        frame_dt: Option<u64>,
        translation: Vec3,
        rotation: Vec3,
    },
    /// Spawns the zengin world `world` (path relative to the worlds directory
    /// without the `.ZEN` extension). Replays load no world unless they
    /// request it with this command.
    ZenginSpawnWorld {
        #[serde(skip_serializing_if = "Option::is_none")]
        frame: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        frame_dt: Option<u64>,
        world: String,
    },
    /// Adds name filters for npc spawning. An npc is spawned when its script
    /// symbol name or display name contains any of `filters`
    /// (case-insensitive). Repeated commands extend the same list.
    ZenginFilterNpcWithName {
        #[serde(skip_serializing_if = "Option::is_none")]
        frame: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        frame_dt: Option<u64>,
        filters: Vec<String>,
    },
    /// Restricts spawning to the union of the given circles: an entity is
    /// spawned only when its position is within `radius` world units of a
    /// point. Repeated commands extend the same list. The world mesh,
    /// waypoint navigation nodes and the player are not filtered.
    ZenginFilterSpawnWithPosition {
        #[serde(skip_serializing_if = "Option::is_none")]
        frame: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        frame_dt: Option<u64>,
        points: Vec<SpawnPoint>,
    },
    /// Selects the npc traced by the debug npc trace, matched as a
    /// case-insensitive substring of its script symbol name. A blank `name`
    /// disables the trace.
    ZenginSetDebugNpc {
        #[serde(skip_serializing_if = "Option::is_none")]
        frame: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        frame_dt: Option<u64>,
        name: String,
    },
    /// Spawns the test world `name` defined in code, with its own camera.
    /// Does not load a zengin world archive.
    TestWorld {
        #[serde(skip_serializing_if = "Option::is_none")]
        frame: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        frame_dt: Option<u64>,
        name: String,
    },
    /// Exits the application.
    ExitGame {
        #[serde(skip_serializing_if = "Option::is_none")]
        frame: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        frame_dt: Option<u64>,
    },
    /// Panics when no captured log line matches the regex.
    LogExists {
        #[serde(skip_serializing_if = "Option::is_none")]
        frame: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        frame_dt: Option<u64>,
        regex: String,
    },
    /// Clears the captured log lines.
    ClearLogBuffer {
        #[serde(skip_serializing_if = "Option::is_none")]
        frame: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        frame_dt: Option<u64>,
    },
}

/// Default offset used when a command has neither `frame` nor `frame_dt`.
pub(crate) const DEFAULT_FRAME_DT: u64 = 5;

impl ReplayCommand {
    #[allow(
        clippy::unnecessary_rest_pattern,
        reason = "a single pattern group keeps the shared timing fields together"
    )]
    pub(crate) fn timing(&self) -> (Option<u64>, Option<u64>) {
        match *self {
            Self::Press {
                frame, frame_dt, ..
            }
            | Self::Release {
                frame, frame_dt, ..
            }
            | Self::Axis {
                frame, frame_dt, ..
            }
            | Self::TeleportPlayer {
                frame, frame_dt, ..
            }
            | Self::SetCameraLocation {
                frame, frame_dt, ..
            }
            | Self::ZenginEquipWeapon {
                frame, frame_dt, ..
            }
            | Self::ZenginSetGameTimeSpeed {
                frame, frame_dt, ..
            }
            | Self::ZenginToggleArmor {
                frame, frame_dt, ..
            }
            | Self::Screenshot {
                frame, frame_dt, ..
            }
            | Self::ZenginSpawnPlayerCharacter {
                frame, frame_dt, ..
            }
            | Self::ZenginSpawnWorld {
                frame, frame_dt, ..
            }
            | Self::ZenginFilterNpcWithName {
                frame, frame_dt, ..
            }
            | Self::ZenginFilterSpawnWithPosition {
                frame, frame_dt, ..
            }
            | Self::ZenginSetDebugNpc {
                frame, frame_dt, ..
            }
            | Self::ZenginGiveTestItems {
                frame, frame_dt, ..
            }
            | Self::ZenginGiveTestJournal {
                frame, frame_dt, ..
            }
            | Self::TestWorld {
                frame, frame_dt, ..
            }
            | Self::OpenMainMenu {
                frame, frame_dt, ..
            }
            | Self::LogExists {
                frame, frame_dt, ..
            }
            | Self::ClearLogBuffer {
                frame, frame_dt, ..
            }
            | Self::ExitGame {
                frame, frame_dt, ..
            } => (frame, frame_dt),
        }
    }
}

/// In-memory recording, rewritten to the target file whenever new commands
/// appear.
#[derive(Resource)]
struct ActionRecording {
    path: PathBuf,
    frame: u64,
    commands: Vec<ReplayCommand>,
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
    let Some(path) = options.save_actions() else {
        return;
    };
    if let Err(error) = fs::remove_file(path)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        warn!("Failed to remove old action recording {path:?}: {error}");
    }
    commands.insert_resource(ActionRecording {
        path: path.clone(),
        frame: 0,
        // Recorded scenarios need an explicit world spawn to replay in a world.
        commands: vec![ReplayCommand::ZenginSpawnWorld {
            frame: Some(0),
            frame_dt: None,
            world: START_WORLD.to_string(),
        }],
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
    let Some(recording) = recording.as_deref_mut() else {
        return;
    };
    recording.frame += 1;
    let frame = recording.frame;

    let before = recording.commands.len();

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
        commands.push(ReplayCommand::TeleportPlayer {
            frame: Some(frame),
            frame_dt: None,
            translation,
            rotation: player_rotation,
        });
        commands.push(ReplayCommand::SetCameraLocation {
            frame: Some(frame),
            frame_dt: None,
            translation,
            rotation: camera_rotation,
        });
    }

    recording.commands = commands;
    if recording.commands.len() != before {
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
    commands: &mut Vec<ReplayCommand>,
    held: &mut HashSet<A>,
) {
    let Some(state) = state else {
        return;
    };
    for action in state.get_just_pressed() {
        if held.insert(action) {
            commands.push(ReplayCommand::Press {
                frame: Some(frame),
                frame_dt: None,
                action: to_recorded(action),
            });
        }
    }
    held.retain(|action| {
        if state.pressed(action) {
            return true;
        }
        commands.push(ReplayCommand::Release {
            frame: Some(frame),
            frame_dt: None,
            action: to_recorded(*action),
        });
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
    commands: &mut Vec<ReplayCommand>,
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
    commands: &mut Vec<ReplayCommand>,
    last_axis: &mut HashMap<RecordedAction, Vec2>,
) {
    let unchanged = last_axis
        .get(&action)
        .is_some_and(|last| last.distance(value) < 0.001);
    if unchanged {
        return;
    }
    last_axis.insert(action, value);
    commands.push(ReplayCommand::Axis {
        frame: Some(frame),
        frame_dt: None,
        action,
        value,
    });
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
        .to_string_pretty(&recording.commands, pretty_config)
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
            "// Recorded replay commands, replayable with --replay-actions"
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
