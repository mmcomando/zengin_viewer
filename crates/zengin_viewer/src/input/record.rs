//! Recording of triggered actions into a RON file, enabled by `--save-actions`.
//!
//! The recording is a frame-indexed command stream (`ReplayCommand`) that can
//! be replayed later with `--replay-actions` (see `input/replay.rs`).

use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;

use bevy::prelude::*;
use leafwing_input_manager::prelude::{ActionState, Actionlike};
use serde::{Deserialize, Serialize};

use crate::input::actions::{
    DebugAction, DebugInputMarker, GameplayAction, GameplayInputMarker, GuiAction, GuiInputMarker,
};
use crate::input::options::Options;

/// Records every triggered action together with its frame number.
pub struct ActionRecordingPlugin;

impl Plugin for ActionRecordingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, open_recording_file);
        app.add_systems(Update, record_triggered_actions);
    }
}

/// A recorded action, tagged with the input map it belongs to.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum RecordedAction {
    Gameplay(GameplayAction),
    Gui(GuiAction),
    Debug(DebugAction),
}

/// One recorded replay command. Together these fully describe the triggered
/// inputs and extra commands of a run so they can be replayed frame by frame.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub(crate) enum ReplayCommand {
    /// Button was pressed on `frame`; replay holds it until a matching
    /// `Release`.
    Press { frame: u64, action: RecordedAction },
    /// Button was released on `frame`.
    Release { frame: u64, action: RecordedAction },
    /// Dual axis value changed on `frame` to `value`.
    Axis {
        frame: u64,
        action: RecordedAction,
        value: Vec2,
    },
    /// Teleport the player character on `frame` to `translation` with
    /// `rotation` in degrees around the XYZ axes.
    Teleport {
        frame: u64,
        translation: Vec3,
        rotation: Vec3,
    },
    /// Save a screenshot of the primary window to `path` on `frame`.
    Screenshot { frame: u64, path: String },
}

impl ReplayCommand {
    pub(crate) fn frame(&self) -> u64 {
        match *self {
            ReplayCommand::Press { frame, .. }
            | ReplayCommand::Release { frame, .. }
            | ReplayCommand::Axis { frame, .. }
            | ReplayCommand::Teleport { frame, .. }
            | ReplayCommand::Screenshot { frame, .. } => frame,
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
    held_debug: HashSet<DebugAction>,
    /// Last recorded dual axis values, to detect changes.
    last_axis: HashMap<RecordedAction, Vec2>,
}

/// Removes any previous recording file.
fn open_recording_file(options: Res<Options>, mut commands: Commands) {
    let Some(path) = options.save_actions() else {
        return;
    };
    if let Err(error) = fs::remove_file(path) {
        if error.kind() != std::io::ErrorKind::NotFound {
            warn!("Failed to remove old action recording {path:?}: {error}");
        }
    }
    commands.insert_resource(ActionRecording {
        path: path.clone(),
        frame: 0,
        commands: Vec::new(),
        held_gameplay: HashSet::new(),
        held_gui: HashSet::new(),
        held_debug: HashSet::new(),
        last_axis: HashMap::new(),
    });
}

/// Collects input events of the current frame and saves the recording file.
fn record_triggered_actions(
    mut recording: Option<ResMut<ActionRecording>>,
    gameplay: Option<Single<&ActionState<GameplayAction>, With<GameplayInputMarker>>>,
    gui: Option<Single<&ActionState<GuiAction>, With<GuiInputMarker>>>,
    debug: Option<Single<&ActionState<DebugAction>, With<DebugInputMarker>>>,
) {
    let Some(recording) = recording.as_deref_mut() else {
        return;
    };
    recording.frame += 1;
    let frame = recording.frame;

    let before = recording.commands.len();

    let gameplay = gameplay.map(|state| state.into_inner());
    let gui = gui.map(|state| state.into_inner());
    let debug = debug.map(|state| state.into_inner());

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
        debug,
        RecordedAction::Debug,
        frame,
        &mut commands,
        &mut recording.held_debug,
    );

    // The only dual axis action is gameplay `Move`.
    if let Some(state) = gameplay {
        let value = state.axis_pair(&GameplayAction::Move);
        record_axis(
            RecordedAction::Gameplay(GameplayAction::Move),
            value,
            frame,
            &mut commands,
            &mut recording.last_axis,
        );
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
                frame,
                action: to_recorded(action),
            });
        }
    }
    held.retain(|action| {
        if state.pressed(action) {
            return true;
        }
        commands.push(ReplayCommand::Release {
            frame,
            action: to_recorded(*action),
        });
        false
    });
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
        frame,
        action,
        value,
    });
}

/// Rewrites the whole recording file with the current command list.
fn save_recording(recording: &ActionRecording) {
    let contents = match ron::ser::to_string(&recording.commands) {
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
