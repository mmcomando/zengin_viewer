use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

/// Adds [`leafwing_input_manager`] and spawns the global input entity with the
/// default [`PlayerAction`] profile.
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
#[derive(Actionlike, PartialEq, Eq, Clone, Copy, Hash, Debug, Reflect)]
pub enum PlayerAction {
    /// Movement direction from WASD, the arrow keys, or the left gamepad stick.
    #[actionlike(DualAxis)]
    Move,
    Jump,
    ToggleCharacterCamera,
    TeleportOrigin, // VR teleport to fixed location
    TeleportAway,   // VR teleport forward
}

impl PlayerAction {
    /// The default profile, matching the previously hardcoded keybindings.
    ///
    /// This is the single place where inputs are mapped to actions; profiles
    /// can later be swapped or loaded from disk, as `InputMap` is serializable.
    pub fn default_input_map() -> InputMap<Self> {
        InputMap::default()
            .with_dual_axis(Self::Move, VirtualDPad::wasd())
            // Up, down, left, right.
            .with_dual_axis(
                Self::Move,
                VirtualDPad::new(
                    KeyCode::ArrowUp,
                    KeyCode::ArrowDown,
                    KeyCode::ArrowLeft,
                    KeyCode::ArrowRight,
                ),
            )
            .with_dual_axis(Self::Move, GamepadStick::LEFT)
            .with(Self::Jump, KeyCode::Space)
            .with(Self::Jump, GamepadButton::South)
            .with(Self::ToggleCharacterCamera, KeyCode::Digit0)
            .with(Self::TeleportOrigin, KeyCode::Digit1)
            .with(Self::TeleportAway, KeyCode::Digit2)
    }
}

fn spawn_input_map(mut commands: Commands) {
    // Inserting `InputMap` also inserts the required `ActionState` component.
    // No gamepad is assigned to the map, so any connected gamepad is accepted.
    commands.spawn((GlobalInput, PlayerAction::default_input_map()));
}
