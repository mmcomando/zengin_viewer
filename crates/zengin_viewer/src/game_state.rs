use avian3d::prelude::*;
use bevy::prelude::*;

/// Top-level application state. The game world exists only in [`AppState::InGame`].
#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum AppState {
    /// Default startup state: behaves as if the player picked New Game.
    #[default]
    InGame,
    MainMenu,
}

/// Loading phase of an in-game world. Physics and the character controller are
/// frozen until the world mesh and its colliders are available, so the player
/// does not fall through the not-yet-spawned world.
#[derive(SubStates, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[source(AppState = AppState::InGame)]
pub(crate) enum WorldLoadState {
    #[default]
    Loading,
    Ready,
}

pub(crate) struct GameStatePlugin;

impl Plugin for GameStatePlugin {
    fn build(&self, app: &mut App) {
        app.add_sub_state::<WorldLoadState>();
        app.add_systems(OnEnter(AppState::MainMenu), pause_physics);
        app.add_systems(OnExit(AppState::MainMenu), resume_physics);
        app.add_systems(OnEnter(WorldLoadState::Loading), pause_physics);
        app.add_systems(OnExit(WorldLoadState::Loading), resume_physics);
    }
}

fn pause_physics(mut time: ResMut<Time<Physics>>) {
    time.pause();
}

fn resume_physics(mut time: ResMut<Time<Physics>>) {
    time.unpause();
}
