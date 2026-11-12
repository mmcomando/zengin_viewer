use avian3d::prelude::*;
use bevy::prelude::*;

/// Top-level application state. The game world exists in [`AppState::InGame`]
/// and is being built in [`AppState::LoadScreen`].
#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum AppState {
    /// The world is fully loaded and gameplay systems run.
    #[default]
    InGame,
    MainMenu,
    /// The world is being spawned or restored. Physics is frozen and the
    /// character controller does not run until the transition to
    /// [`AppState::InGame`].
    LoadScreen,
}

/// Whether a game world finished loading and is safe to inspect or save.
/// Unlike the state it is not cleared by opening the main menu, so an in-game
/// save request made from the menu still sees a loaded world.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub(crate) struct WorldLoaded(pub(crate) bool);

/// Run condition for systems that require a fully loaded world.
pub(crate) fn world_is_loaded(loaded: Res<WorldLoaded>) -> bool {
    loaded.0
}

pub(crate) struct GameStatePlugin;

impl Plugin for GameStatePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WorldLoaded>();
        app.add_systems(OnEnter(AppState::MainMenu), pause_physics);
        app.add_systems(OnExit(AppState::MainMenu), resume_physics);
        app.add_systems(
            OnEnter(AppState::LoadScreen),
            (pause_physics, mark_world_unloaded),
        );
        app.add_systems(OnExit(AppState::LoadScreen), resume_physics);
        app.add_systems(OnEnter(AppState::InGame), mark_world_loaded);
    }
}

fn pause_physics(mut time: ResMut<Time<Physics>>) {
    time.pause();
}

fn resume_physics(mut time: ResMut<Time<Physics>>) {
    time.unpause();
}

fn mark_world_loaded(mut loaded: ResMut<WorldLoaded>) {
    loaded.0 = true;
}

fn mark_world_unloaded(mut loaded: ResMut<WorldLoaded>) {
    loaded.0 = false;
}
