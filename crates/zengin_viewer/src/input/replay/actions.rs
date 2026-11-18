use bevy::prelude::*;
use leafwing_input_manager::action_state::ActionKindData;
use leafwing_input_manager::buttonlike::ButtonState;
use leafwing_input_manager::prelude::{ActionState, Actionlike};

use crate::game_state::AppState;
use crate::gameplay::game_time::GameTime;
use crate::gameplay::objects::PlayerCharacter;
use crate::gui::GameLog;
use crate::input::actions::{
    EditorAction, EditorInputMarker, GameplayAction, GameplayInputMarker, GuiAction,
    GuiInputMarker, SharedAction, SharedInputMarker,
};
use crate::input::log_capture::LogBuffer;
use crate::input::record::RecordedAction;
use crate::zengin::{GameNpc, NpcSpawnNameFilter, ScriptResource, SpawnPositionFilter};

use super::command::apply_replay_command;
use super::state::{ActionReplay, PendingLocations};

/// Applies the replayed actions of the current frame.
///
/// Held buttons and axes are re-applied every frame, because the input
/// manager recomputes action states from raw device input each frame and
/// would otherwise wipe synthetic changes immediately.
pub(super) fn replay_actions(
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
    for action in std::mem::take(&mut replay.released_buttons) {
        replay.held_buttons.remove(&action);
    }

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
