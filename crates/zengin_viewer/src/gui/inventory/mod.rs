//! Placeholder inventory UI: item slot grids and item info panels. Items are
//! shown as their (truncated) name text instead of 3D models.
//!
// TODO replace item text with 3d models
//!
//! Two reusable "inventory window" panels exist: the trader side on the left
//! and the player side on the right. A normal inventory shows only the player
//! panel, a trade shows both and enables buying/selling.

mod data;
mod input;
mod render;
mod spawn;
mod state;
mod style;

pub(crate) use state::{InventoryUiState, OpenLoot, OpenTrade};

use bevy::prelude::*;

use crate::game_state::AppState;

pub struct InventoryUiPlugin;

impl Plugin for InventoryUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InventoryUiState>();
        app.add_message::<OpenTrade>();
        app.add_message::<OpenLoot>();
        app.add_systems(Startup, spawn::spawn_inventory_ui);
        app.add_systems(
            Update,
            (
                input::open_trade,
                input::open_loot,
                input::toggle_inventory,
                input::handle_inventory_input,
                input::close_orphaned_trade,
                render::sync_inventory_ui,
            )
                .chain()
                .run_if(in_state(AppState::InGame)),
        );
    }
}
