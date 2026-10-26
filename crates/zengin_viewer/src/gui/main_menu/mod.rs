use bevy::prelude::*;

use crate::game_state::AppState;
use crate::zengin::MenuScriptResource;

mod input;
mod render;
mod spawn;
mod state;
mod style;

pub(crate) use state::MainMenuState;

use state::{MainMenuData, initial_selection};

/// Main menu driven by the `menu_main` instance of MENU.DAT.
pub struct MainMenuPlugin;

impl Plugin for MainMenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MainMenuState>();
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
    world.insert_resource(script);
}

fn open_main_menu(mut state: ResMut<MainMenuState>, data: Option<Res<MainMenuData>>) {
    let selected = data
        .as_ref()
        .and_then(|data| data.0.as_ref())
        .and_then(initial_selection)
        .unwrap_or(0);
    state.open(selected);
    info!("Main menu opened");
}

fn close_main_menu(mut state: ResMut<MainMenuState>) {
    state.close();
    info!("Main menu closed");
}
