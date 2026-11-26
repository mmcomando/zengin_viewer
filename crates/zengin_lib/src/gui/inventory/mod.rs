//! Placeholder inventory UI: item slot grids and item info panels. Items are
//! shown as their (truncated) name text instead of 3D models.
//!
// TODO replace item text with 3d models
//!
//! Two reusable "inventory window" panels exist: the trader side on the left
//! and the player side on the right. A normal inventory shows only the player
//! panel, a trade shows both and enables buying/selling.

pub(crate) mod data;
pub(crate) mod input;
pub(crate) mod render;
pub(crate) mod spawn;
pub(crate) mod state;
pub(crate) mod style;

pub use state::{
    InventoryUiState, LockpickOpened, LootClosed, LootSource, OpenLockpick, OpenLoot, OpenTrade,
};

use bevy::prelude::*;

use crate::AppState;

pub struct InventoryUiPlugin;

impl Plugin for InventoryUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InventoryUiState>();
        app.add_message::<OpenTrade>();
        app.add_message::<OpenLoot>();
        app.add_message::<OpenLockpick>();
        app.add_message::<LockpickOpened>();
        app.add_message::<LootClosed>();
        app.add_systems(Startup, spawn::spawn_inventory_ui);
        app.add_systems(
            Update,
            (
                input::open_trade,
                input::open_loot,
                input::open_lockpick,
                input::toggle_inventory,
                input::handle_lockpick_input,
                input::handle_inventory_input,
                input::close_orphaned_trade,
                input::emit_loot_closed,
            )
                .chain()
                .run_if(in_state(AppState::InGame)),
        );
        app.add_systems(
            Update,
            render::sync_inventory_ui.after(input::close_orphaned_trade),
        );
    }
}
