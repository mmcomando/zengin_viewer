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

/// Loading phase of the game world. Physics and the character controller are
/// frozen until the world mesh and its colliders are available, so the player
/// does not fall through the not-yet-spawned world. This is a top-level state so
/// its value survives the main menu, where it is used to tell whether saving is
/// currently allowed.
#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum LoadScreen {
    #[default]
    Loading,
    Ready,
}

pub(crate) struct GameStatePlugin;

impl Plugin for GameStatePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<LoadScreen>();
        app.add_systems(OnEnter(AppState::MainMenu), pause_physics);
        // Leaving the menu while the world is still loading keeps physics paused
        // until `LoadScreen::Ready` resumes it.
        app.add_systems(
            OnExit(AppState::MainMenu),
            resume_physics.run_if(in_state(LoadScreen::Ready)),
        );
        app.add_systems(OnEnter(LoadScreen::Loading), pause_physics);
        app.add_systems(OnExit(LoadScreen::Loading), resume_physics);
    }
}

fn pause_physics(mut time: ResMut<Time<Physics>>) {
    time.pause();
}

fn resume_physics(mut time: ResMut<Time<Physics>>) {
    time.unpause();
}
