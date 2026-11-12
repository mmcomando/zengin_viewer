//! Stats (character sheet) UI, driven by the `menu_status` screen of MENU.DAT.

mod data;
mod input;
mod render;
mod spawn;
mod state;
mod style;

pub(crate) use state::StatsUiState;

use bevy::prelude::*;

use crate::game_state::AppState;
use crate::zengin::ensure_menu_script;

use data::StatsMenuData;

pub(crate) struct StatsUiPlugin;

impl Plugin for StatsUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<StatsUiState>();
        app.add_systems(Startup, spawn::spawn_stats_ui);
        app.add_systems(OnEnter(AppState::InGame), load_stats_menu_data);
        app.add_systems(
            Update,
            (input::toggle_stats, input::close_stats_on_cancel)
                .chain()
                .run_if(in_state(AppState::InGame)),
        );
        app.add_systems(
            Update,
            render::sync_stats_ui.after(input::close_stats_on_cancel),
        );
    }
}

/// Projects `menu_status` once when entering the game. The menu script may
/// already be loaded by the main menu, otherwise this creates it.
fn load_stats_menu_data(world: &mut World) {
    let Some(script) = ensure_menu_script(world) else {
        error!("Status menu unavailable: MENU.DAT could not be loaded");
        world.insert_resource(StatsMenuData(None));
        return;
    };

    let definition = script.load_section("menu_status");
    if definition.is_none() {
        error!("Status menu unavailable: menu_status could not be projected");
    }

    let data = StatsMenuData(definition);
    info!(
        "Status menu loaded with {} item(s)",
        data.definition().map_or(0, |menu| menu.items.len())
    );
    world.insert_resource(data);
}
