//! Replay of recorded actions from a RON file, enabled by `--replay-actions`.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use bevy::prelude::*;
use bevy::render::view::window::screenshot::{Screenshot, save_to_disk};
use leafwing_input_manager::action_state::ActionKindData;
use leafwing_input_manager::buttonlike::ButtonState;
use leafwing_input_manager::plugin::InputManagerSystem;
use leafwing_input_manager::prelude::{ActionState, Actionlike};

use crate::gameplay::character::PlayerPhysicsBodyMarker;
use crate::input::actions::{
    DebugAction, DebugInputMarker, GameplayAction, GameplayInputMarker, GuiAction, GuiInputMarker,
    update_input_mode,
};
use crate::input::options::Options;
use crate::input::record::{RecordedAction, ReplayCommand};

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
    }
}

/// Replay schedule loaded from the recording file plus the replay state.
#[derive(Resource)]
struct ActionReplay {
    /// Events sorted by frame.
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

/// Loads and prepares the recording file for replay.
fn load_replay_file(options: Res<Options>, mut commands: Commands) {
    let Some(path) = options.replay_actions() else {
        return;
    };
    let replay = match read_replay(path) {
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

fn read_replay(path: &Path) -> Result<ActionReplay, String> {
    let contents =
        fs::read_to_string(path).map_err(|error| format!("failed to read {path:?}: {error}"))?;
    let mut commands: Vec<ReplayCommand> = ron::de::from_str(&contents)
        .map_err(|error| format!("failed to parse {path:?}: {error}"))?;
    commands.sort_by_key(|command| command.frame());
    Ok(ActionReplay {
        commands,
        next: 0,
        frame: 0,
        held_buttons: HashSet::new(),
        started_buttons: HashSet::new(),
        held_axes: HashMap::new(),
    })
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
    mut debug: Query<&mut ActionState<DebugAction>, With<DebugInputMarker>>,
    mut player: Query<&mut Transform, With<PlayerPhysicsBodyMarker>>,
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
            ReplayCommand::Teleport {
                translation,
                rotation,
                ..
            } => {
                teleport_player(&mut player, *translation, *rotation);
            }
            ReplayCommand::Screenshot { path, .. } => {
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_to_disk(path.clone()));
            }
        }
    }

    for action in replay.held_buttons.iter() {
        let started = replay.started_buttons.contains(action);
        press(&mut gameplay, &mut gui, &mut debug, *action, started);
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

/// Teleports the player character to `translation` with `rotation` given in
/// degrees around the XYZ axes.
fn teleport_player(
    player: &mut Query<&mut Transform, With<PlayerPhysicsBodyMarker>>,
    translation: Vec3,
    rotation: Vec3,
) {
    let Ok(mut transform) = player.single_mut() else {
        warn!("Replay teleport failed: no player character found");
        return;
    };
    transform.translation = translation;
    transform.rotation = Quat::from_euler(
        EulerRot::XYZ,
        rotation.x.to_radians(),
        rotation.y.to_radians(),
        rotation.z.to_radians(),
    );
}
