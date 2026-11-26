use bevy::prelude::*;
use leafwing_input_manager::prelude::ActionState;

use crate::game_state::AppState;
use crate::gameplay::game_time::PlayTime;
use crate::input::{GuiAction, GuiInputMarker, SharedAction, SharedInputMarker};
use crate::zengin::save::{LoadSaveRequest, QuickSave};
use crate::zengin::{
    CurrentWorld, MenuDefinition, MenuItemDefinition, NewGame, SEL_ACTION_BACK, SEL_ACTION_CLOSE,
    SEL_ACTION_START_MENU,
};

use super::state::{
    LoadMenuData, MainMenuData, MainMenuScreen, MainMenuState, is_load_back, load_selectable_slots,
    load_slot_index, selectable_slots,
};
use dev_util::perf_debug;

#[allow(clippy::too_many_arguments)]
pub(super) fn handle_main_menu_input(
    action_state: Single<&ActionState<GuiAction>, With<GuiInputMarker>>,
    shared_actions: Single<&ActionState<SharedAction>, With<SharedInputMarker>>,
    data: Option<Res<MainMenuData>>,
    load_data: Option<Res<LoadMenuData>>,
    current_world: Res<CurrentWorld>,
    mut state: ResMut<MainMenuState>,
    mut next_state: ResMut<NextState<AppState>>,
    mut play_time: ResMut<PlayTime>,
    mut quick_save: MessageWriter<QuickSave>,
    mut new_game: MessageWriter<NewGame>,
    mut load_request: MessageWriter<LoadSaveRequest>,
    mut exit: MessageWriter<AppExit>,
    mut commands: Commands,
) {
    let _perf = perf_debug!();
    if !state.open {
        return;
    }
    let in_game = !current_world.0.is_empty();
    let back_pressed = action_state.just_pressed(&GuiAction::CancelAction)
        || shared_actions.just_pressed(&SharedAction::Back);
    if state.screen == MainMenuScreen::Load && back_pressed {
        return_to_main(&mut state, data.as_ref().and_then(|data| data.0.as_ref()));
        return;
    }
    let selectable = match state.screen {
        MainMenuScreen::Main => {
            let Some(definition) = data.as_ref().and_then(|data| data.0.as_ref()) else {
                return;
            };
            selectable_slots(definition, in_game)
        }
        MainMenuScreen::Load => {
            let Some(definition) = load_data.as_ref().and_then(|data| data.0.as_ref()) else {
                return;
            };
            load_selectable_slots(definition, &state.saved_games)
        }
    };
    if selectable.is_empty() {
        return;
    }
    move_selection_on_input(&action_state, &mut state.selected_slot, &selectable);

    if !action_state.just_pressed(&GuiAction::Confirm) {
        return;
    }

    match state.screen {
        MainMenuScreen::Main => {
            let Some(definition) = data.as_ref().and_then(|data| data.0.as_ref()) else {
                return;
            };
            let Some(item) = state.selected_item(definition).cloned() else {
                return;
            };
            handle_main_item(
                &item,
                in_game,
                load_data.as_ref().and_then(|data| data.0.as_ref()),
                &mut state,
                &mut next_state,
                &mut play_time,
                &mut quick_save,
                &mut new_game,
                &mut exit,
                &mut commands,
            );
        }
        MainMenuScreen::Load => {
            let Some(definition) = load_data.as_ref().and_then(|data| data.0.as_ref()) else {
                return;
            };
            let Some(item) = state.selected_item(definition).cloned() else {
                return;
            };
            handle_load_item(
                &item,
                definition,
                data.as_ref().and_then(|data| data.0.as_ref()),
                &mut state,
                &mut load_request,
            );
        }
    }
}

fn handle_main_item(
    item: &MenuItemDefinition,
    in_game: bool,
    load: Option<&MenuDefinition>,
    state: &mut MainMenuState,
    next_state: &mut NextState<AppState>,
    play_time: &mut PlayTime,
    quick_save: &mut MessageWriter<QuickSave>,
    new_game: &mut MessageWriter<NewGame>,
    exit: &mut MessageWriter<AppExit>,
    commands: &mut Commands,
) {
    for (action, argument) in &item.on_sel_action {
        match (*action, argument.as_str()) {
            (SEL_ACTION_START_MENU, "MENU_SAVEGAME_SAVE") => {
                info!("Main menu: saving the game");
                quick_save.write(QuickSave);
            }
            (SEL_ACTION_START_MENU, "MENU_SAVEGAME_LOAD") => {
                open_load_screen(state, load);
            }
            (SEL_ACTION_CLOSE, "NEW_GAME") => {
                start_new_game(in_game, play_time, new_game, next_state, commands);
            }
            _ => apply_action(*action, argument, next_state, exit),
        }
    }
    if item.on_event_action.iter().any(Option::is_some) {
        warn!("Main menu event actions are not implemented yet");
    }
}

fn start_new_game(
    in_game: bool,
    play_time: &mut PlayTime,
    new_game: &mut MessageWriter<NewGame>,
    next_state: &mut NextState<AppState>,
    commands: &mut Commands,
) {
    info!("Main menu: starting new game");
    play_time.reset();
    if in_game {
        new_game.write(NewGame);
        return;
    }
    // The game script is not loaded yet, so the default world spawn is only
    // queued here; it runs once the loading screen has the script ready.
    commands.queue(|world: &mut World| crate::zengin::request_initial_world(world));
    next_state.set(AppState::LoadScreen);
}

fn handle_load_item(
    item: &MenuItemDefinition,
    definition: &MenuDefinition,
    main: Option<&MenuDefinition>,
    state: &mut MainMenuState,
    load_request: &mut MessageWriter<LoadSaveRequest>,
) {
    if is_load_back(item) {
        return_to_main(state, main);
        return;
    }
    let Some(index) = load_slot_index(definition, item.slot) else {
        warn!("Load menu item {} is not a save slot", item.name);
        return;
    };
    let Some(entry) = state.saved_games.get(index) else {
        return;
    };
    info!("Main menu: loading {}", entry.path.display());
    load_request.write(LoadSaveRequest {
        path: entry.path.clone(),
    });
}

fn open_load_screen(state: &mut MainMenuState, load: Option<&MenuDefinition>) {
    let Some(definition) = load else {
        error!("Load menu unavailable: MENU_SAVEGAME_LOAD could not be loaded");
        return;
    };
    state.open_load_screen(0);
    info!(
        "Main menu: opened load screen with {} save(s)",
        state.saved_games.len()
    );
    let selectable = load_selectable_slots(definition, &state.saved_games);
    if let Some(first) = selectable.first() {
        state.selected_slot = *first;
    }
}

fn return_to_main(state: &mut MainMenuState, main: Option<&MenuDefinition>) {
    let selected = main
        .and_then(|definition| {
            definition.items.iter().find(|item| {
                item.on_sel_action.iter().any(|(action, argument)| {
                    *action == SEL_ACTION_START_MENU && argument == "MENU_SAVEGAME_LOAD"
                })
            })
        })
        .map_or(0, |item| item.slot);
    state.open_main_screen(selected);
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

#[expect(
    clippy::cognitive_complexity,
    reason = "Dispatch mirrors the menu script's action/argument pairs"
)]
fn apply_action(
    action: i32,
    argument: &str,
    next_state: &mut NextState<AppState>,
    exit: &mut MessageWriter<AppExit>,
) {
    match action {
        SEL_ACTION_CLOSE => apply_close_action(argument, next_state, exit),
        SEL_ACTION_BACK => {
            info!("Main menu: resuming the game");
            next_state.set(AppState::InGame);
        }
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
            next_state.set(AppState::LoadScreen);
        }
        "LEAVE_GAME" => {
            info!("Main menu: exiting");
            exit.write(AppExit::Success);
        }
        other => warn!("Main menu CLOSE action {other} is not implemented"),
    }
}
