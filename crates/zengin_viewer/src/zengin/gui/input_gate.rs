//! Gate for player-driven world input while a UI view owns it.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::zengin::DialogUiState;
use crate::zengin::gui::{ChapterScreenState, InventoryUiState, JournalUiState, StatsUiState};

/// The views that own the input while open. Read directly rather than through
/// [`InputMode`](input_core::InputMode), which changes on the next frame, so a
/// view opened in the same frame blocks immediately.
#[derive(SystemParam)]
pub(crate) struct WorldInputViews<'w> {
    dialog: Res<'w, DialogUiState>,
    chapter_screen: Res<'w, ChapterScreenState>,
    inventory: Res<'w, InventoryUiState>,
    journal: Res<'w, JournalUiState>,
    stats: Res<'w, StatsUiState>,
}

impl WorldInputViews<'_> {
    /// Whether player-driven world systems may read gameplay input. False while
    /// a dialog, inventory, journal, stats menu or chapter screen is open.
    pub(crate) fn enabled(&self) -> bool {
        !self.dialog.is_open()
            && !self.chapter_screen.is_open()
            && !self.inventory.is_open()
            && !self.journal.is_open()
            && !self.stats.is_open()
    }
}
