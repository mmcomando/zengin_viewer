use bevy::prelude::*;
use leafwing_input_manager::prelude::*;
use serde::{Deserialize, Serialize};

/// Path of the input profile, relative to the working directory
/// (the same `assets` folder the [`bevy::asset::AssetServer`] reads from).
const INPUT_PROFILE_PATH: &str = "crates/zengin_viewer/assets/input_bindings.ron";

/// Adds [`leafwing_input_manager`] and spawns the global input entity with the
/// input profile loaded from [`INPUT_PROFILE_PATH`], falling back to the
/// built-in [`PlayerAction::default_input_map`] if the file can't be read.
pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(InputManagerPlugin::<PlayerAction>::default());
        app.add_systems(Startup, spawn_input_map);
    }
}

/// Marker for the entity holding the global [`InputMap`] (and its required
/// [`ActionState`]) used by all gameplay and UI systems.
#[derive(Component)]
pub struct GlobalInput;

/// The logical actions of the game, independent of the input device.
///
/// Buttonlike is the default control kind; variants with a different kind are
/// annotated with `#[actionlike(...)]`.
#[derive(Actionlike, PartialEq, Eq, Clone, Copy, Hash, Debug, Reflect, Serialize, Deserialize)]
pub enum PlayerAction {
    /// Movement direction from WASD, the arrow keys, or the left gamepad stick.
    #[actionlike(DualAxis)]
    Move,
    Jump,
    ToggleCharacterCamera,
    TeleportOrigin,
    TeleportAway,
}

fn spawn_input_map(mut commands: Commands) {
    let input_map = load_input_profile().unwrap();
    commands.spawn((GlobalInput, input_map));
}

/// Loads the input profile from [`INPUT_PROFILE_PATH`].
fn load_input_profile() -> Result<InputMap<PlayerAction>, String> {
    let contents = std::fs::read_to_string(INPUT_PROFILE_PATH)
        .map_err(|error| format!("failed to read {INPUT_PROFILE_PATH}: {error}"))?;
    ron::de::from_str(&contents)
        .map_err(|error| format!("failed to parse {INPUT_PROFILE_PATH}: {error}"))
}
