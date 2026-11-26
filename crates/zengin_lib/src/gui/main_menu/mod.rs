use bevy::prelude::*;

use crate::AppState;
use crate::PendingMusic;
use crate::{CurrentWorld, MenuScriptResource};

pub(crate) mod input;
pub(crate) mod render;
pub(crate) mod spawn;
pub(crate) mod state;
pub(crate) mod style;

pub use state::MainMenuState;

use dev_util::perf_debug;
use state::{LoadMenuData, MainMenuData, initial_selection};

/// Main menu driven by the `menu_main` instance of MENU.DAT.
pub struct MainMenuPlugin;

impl Plugin for MainMenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MainMenuState>();
        app.init_resource::<render::SaveThumbnailCache>();
        app.add_systems(Startup, spawn::spawn_main_menu);
        app.add_systems(
            OnEnter(AppState::MainMenu),
            (load_menu_data, open_main_menu).chain(),
        );
        app.add_systems(OnExit(AppState::MainMenu), close_main_menu);
        app.add_systems(
            Update,
            input::handle_main_menu_input.run_if(in_state(AppState::MainMenu)),
        );
        app.add_systems(
            Update,
            render::sync_main_menu.after(input::handle_main_menu_input),
        );
    }
}

/// Loads MENU.DAT and its `menu_main` projection. Runs before the UI is first
/// shown, the loader is exclusive so the resources are visible to the next
/// system in the same `OnEnter`.
fn load_menu_data(world: &mut World) {
    let _perf = perf_debug!();
    let Some(script) = MenuScriptResource::new() else {
        error!("Main menu data unavailable: MENU.DAT could not be loaded");
        world.insert_resource(MainMenuData(None));
        return;
    };
    let definition = script.load_main_menu();
    if definition.is_none() {
        error!("Main menu data unavailable: menu_main could not be projected");
    }
    world.insert_resource(MainMenuData(definition));

    let load_definition = script.load_section("menu_savegame_load");
    if load_definition.is_none() {
        error!("Load menu data unavailable: menu_savegame_load could not be projected");
    }
    world.insert_resource(LoadMenuData(load_definition));
    world.insert_resource(script);
}

fn open_main_menu(
    mut state: ResMut<MainMenuState>,
    data: Option<Res<MainMenuData>>,
    current_world: Res<CurrentWorld>,
    mut pending_music: ResMut<PendingMusic>,
) {
    let _perf = perf_debug!();
    let in_game = !current_world.0.is_empty();
    let menu = data.as_ref().and_then(|data| data.0.as_ref());
    let selected = menu
        .and_then(|data| initial_selection(data, in_game))
        .unwrap_or(0);
    state.open(selected);
    if let Some(theme) = menu.and_then(|menu| menu.music_theme.as_deref()) {
        pending_music.play(theme);
    }
    info!("Main menu opened");
}

fn close_main_menu(mut state: ResMut<MainMenuState>, mut pending_music: ResMut<PendingMusic>) {
    let _perf = perf_debug!();
    state.close();
    pending_music.stop();
    info!("Main menu closed");
}
