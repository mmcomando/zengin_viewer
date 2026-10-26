//! Journal (quest log) UI: category column, topic list and the entry viewer.
//!
//! The topic snapshot is taken from the script when the journal opens; the
//! journal only changes while a dialog runs and dialogs cannot run with the
//! journal open.

mod input;
mod render;
mod spawn;
mod state;
mod style;

pub(crate) use state::JournalUiState;

use bevy::prelude::*;

use crate::game_state::AppState;

pub struct JournalUiPlugin;

impl Plugin for JournalUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<JournalUiState>();
        app.add_systems(Startup, spawn::spawn_journal_ui);
        app.add_systems(
            Update,
            (
                input::toggle_journal,
                input::handle_journal_input,
                render::sync_journal_ui,
            )
                .chain()
                .run_if(in_state(AppState::InGame)),
        );
    }
}
