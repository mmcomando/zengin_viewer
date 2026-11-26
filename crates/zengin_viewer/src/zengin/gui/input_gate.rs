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

pub(crate) fn world_input_enabled(views: WorldInputViews) -> bool {
    views.enabled()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Resource, Default)]
    struct Ran(bool);

    #[test]
    fn no_open_view_allows_world_input() {
        assert!(world_input_runs_with_inventory(false));
    }

    #[test]
    fn open_inventory_blocks_world_input() {
        assert!(!world_input_runs_with_inventory(true));
    }

    fn world_input_runs_with_inventory(open: bool) -> bool {
        let mut app = App::new();
        app.init_resource::<DialogUiState>();
        app.init_resource::<ChapterScreenState>();
        app.init_resource::<InventoryUiState>();
        app.init_resource::<JournalUiState>();
        app.init_resource::<StatsUiState>();
        app.init_resource::<Ran>();
        app.world_mut().resource_mut::<InventoryUiState>().open = open;
        app.add_systems(
            Update,
            (|mut ran: ResMut<Ran>| ran.0 = true).run_if(world_input_enabled),
        );
        app.update();
        app.world().resource::<Ran>().0
    }
}
