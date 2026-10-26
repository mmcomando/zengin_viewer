use bevy::prelude::*;
use leafwing_input_manager::prelude::ActionState;

use crate::gui::InventoryUiState;
use crate::input::{GuiAction, GuiInputMarker, SharedAction, SharedInputMarker};
use crate::zengin::{DialogUiState, ScriptResource};

use super::state::{JournalFocus, JournalUiState};

/// The dialog UI and the inventory take precedence over the journal.
pub(super) fn toggle_journal(
    action_state: Single<&ActionState<SharedAction>, With<SharedInputMarker>>,
    dialog_ui: Res<DialogUiState>,
    inventory_ui: Res<InventoryUiState>,
    mut state: ResMut<JournalUiState>,
    gothic_script: Res<ScriptResource>,
) {
    if !action_state.just_pressed(&SharedAction::Journal) {
        return;
    }
    if state.open {
        state.close();
        return;
    }
    if dialog_ui.is_open() || inventory_ui.is_open() {
        return;
    }

    let topics = gothic_script.lock().journal_topics();
    let count = topics.len();
    state.open(topics);
    info!("Journal opened with {count} topic(s)");
}

pub(super) fn handle_journal_input(
    action_state: Single<&ActionState<GuiAction>, With<GuiInputMarker>>,
    mut state: ResMut<JournalUiState>,
) {
    if !state.open {
        return;
    }

    if state.detail_open {
        if action_state.just_pressed(&GuiAction::CancelAction) {
            state.detail_open = false;
            state.detail_scroll = 0;
            return;
        }
        if action_state.just_pressed(&GuiAction::Up) {
            state.detail_scroll = state.detail_scroll.saturating_sub(1);
        }
        if action_state.just_pressed(&GuiAction::Down) {
            state.detail_scroll += 1;
        }
        return;
    }

    if action_state.just_pressed(&GuiAction::CancelAction) {
        match state.focus {
            JournalFocus::Topics => state.focus = JournalFocus::Categories,
            JournalFocus::Categories => state.close(),
        }
        return;
    }

    match state.focus {
        JournalFocus::Categories => {
            if action_state.just_pressed(&GuiAction::Up) {
                state.move_category(-1);
            }
            if action_state.just_pressed(&GuiAction::Down) {
                state.move_category(1);
            }
            if action_state.just_pressed(&GuiAction::Right)
                || action_state.just_pressed(&GuiAction::Confirm)
            {
                state.focus = JournalFocus::Topics;
            }
        }
        JournalFocus::Topics => {
            if action_state.just_pressed(&GuiAction::Left) {
                state.focus = JournalFocus::Categories;
            }
            if action_state.just_pressed(&GuiAction::Up) {
                state.selected = state.selected.saturating_sub(1);
            }
            if action_state.just_pressed(&GuiAction::Down)
                && state.selected + 1 < state.active.len()
            {
                state.selected += 1;
            }
            if action_state.just_pressed(&GuiAction::Confirm) && !state.active.is_empty() {
                state.detail_open = true;
                state.detail_scroll = 0;
                state.detail_dirty = true;
            }
        }
    }
}
