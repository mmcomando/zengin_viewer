use bevy::prelude::*;
use leafwing_input_manager::prelude::ActionState;

use crate::game_state::AppState;
use crate::input::{GuiAction, GuiInputMarker};
use crate::zengin::{MenuDefinition, MenuItemDefinition, SEL_ACTION_CLOSE, SEL_ACTION_START_MENU};

use super::state::{MainMenuData, MainMenuState, can_select};

pub(super) fn handle_main_menu_input(
    action_state: Single<&ActionState<GuiAction>, With<GuiInputMarker>>,
    data: Option<Res<MainMenuData>>,
    mut state: ResMut<MainMenuState>,
    mut next_state: ResMut<NextState<AppState>>,
    mut exit: MessageWriter<AppExit>,
) {
    if !state.open {
        return;
    }
    let Some(data) = data.as_ref().and_then(|data| data.0.as_ref()) else {
        return;
    };
    let selectable = selectable_slots(data);
    if selectable.is_empty() {
        return;
    }

    move_selection_on_input(&action_state, &mut state.selected_slot, &selectable);
    if !action_state.just_pressed(&GuiAction::Confirm) {
        return;
    }

    let Some(item) = state.selected_item(data) else {
        return;
    };
    apply_item_actions(item, &mut next_state, &mut exit);
}

fn selectable_slots(data: &MenuDefinition) -> Vec<usize> {
    data.items
        .iter()
        .filter(|item| can_select(item))
        .map(|item| item.slot)
        .collect()
}

fn move_selection_on_input(
    action_state: &ActionState<GuiAction>,
    selected: &mut usize,
    selectable: &[usize],
) {
    if action_state.just_pressed(&GuiAction::Up) {
        move_selection(selected, selectable, -1);
    }
    if action_state.just_pressed(&GuiAction::Down) {
        move_selection(selected, selectable, 1);
    }
}

fn move_selection(selected: &mut usize, selectable: &[usize], delta: isize) {
    let index = selectable
        .iter()
        .position(|slot| *slot == *selected)
        .unwrap_or(0);
    let next = (index as isize + delta).rem_euclid(selectable.len() as isize) as usize;
    *selected = selectable[next];
}

fn apply_item_actions(
    item: &MenuItemDefinition,
    next_state: &mut NextState<AppState>,
    exit: &mut MessageWriter<AppExit>,
) {
    for (action, argument) in &item.on_sel_action {
        apply_action(*action, argument, next_state, exit);
    }
    if item.on_event_action.iter().any(Option::is_some) {
        warn!("Main menu event actions are not implemented yet");
    }
}

fn apply_action(
    action: i32,
    argument: &str,
    next_state: &mut NextState<AppState>,
    exit: &mut MessageWriter<AppExit>,
) {
    match action {
        SEL_ACTION_CLOSE => apply_close_action(argument, next_state, exit),
        SEL_ACTION_START_MENU if argument == "MENU_LEAVE_GAME" => {
            info!("Main menu: exiting");
            exit.write(AppExit::Success);
        }
        SEL_ACTION_START_MENU => warn!("Main menu section {argument} is not implemented"),
        other => warn!("Main menu select action {other} is not implemented"),
    }
}

fn apply_close_action(
    argument: &str,
    next_state: &mut NextState<AppState>,
    exit: &mut MessageWriter<AppExit>,
) {
    match argument {
        "NEW_GAME" => {
            info!("Main menu: starting new game");
            next_state.set(AppState::InGame);
        }
        "LEAVE_GAME" => {
            info!("Main menu: exiting");
            exit.write(AppExit::Success);
        }
        other => warn!("Main menu CLOSE action {other} is not implemented"),
    }
}
