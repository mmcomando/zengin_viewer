//! Journal (quest log) UI, driven by the `menu_log` screen of MENU.DAT.

pub mod data;
pub mod input;
pub mod render;
pub mod spawn;
pub mod state;
pub mod style;

pub use state::JournalUiState;

use bevy::prelude::*;

use crate::AppState;
use crate::ensure_menu_script;

use data::JournalMenuData;
use dev_util::perf_debug;

pub struct JournalUiPlugin;

impl Plugin for JournalUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<JournalMenuData>();
        app.init_resource::<JournalUiState>();
        app.add_systems(Startup, spawn::spawn_journal_root);
        app.add_systems(OnEnter(AppState::InGame), load_journal_menu_data);
        app.add_systems(
            Update,
            (input::toggle_journal, input::handle_journal_input)
                .chain()
                .run_if(in_state(AppState::InGame)),
        );
        app.add_systems(
            Update,
            render::sync_journal_ui.after(input::handle_journal_input),
        );
    }
}

/// Projects `menu_log` once when entering the game. The menu script may already
/// be loaded by the main menu or the stats preload.
fn load_journal_menu_data(world: &mut World) {
    let _perf = perf_debug!();
    let Some(script) = ensure_menu_script(world) else {
        error!("Journal menu unavailable: MENU.DAT could not be loaded");
        world.insert_resource(JournalMenuData::default());
        return;
    };

    let definition = script.load_section("menu_log");
    if definition.is_none() {
        error!("Journal menu unavailable: menu_log could not be projected");
    }
    let viewer_back_pic = script.lock().string_variable("log_viewer_back_pic");
    let data = JournalMenuData {
        definition,
        viewer_back_pic,
    };
    info!(
        "Journal menu loaded with {} item(s)",
        data.definition().map_or(0, |menu| menu.items.len())
    );
    world.insert_resource(data);
}
