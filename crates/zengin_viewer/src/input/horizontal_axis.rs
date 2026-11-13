use bevy::prelude::*;
use leafwing_input_manager::clashing_inputs::BasicInputs;
use leafwing_input_manager::prelude::*;
use leafwing_input_manager::user_input::updating::CentralInputStore;
use serde::{Deserialize, Serialize};

/// A horizontal dual axis built from two buttons; the vertical component is
/// always zero.
///
/// `VirtualDPad` needs a button for every direction, so it cannot add extra
/// horizontal-only keys to an axis action.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize)]
pub(super) struct HorizontalAxis {
    /// Button for the negative (left) direction.
    pub negative: Box<dyn Buttonlike>,
    /// Button for the positive (right) direction.
    pub positive: Box<dyn Buttonlike>,
    /// A processing pipeline that handles input values.
    pub processors: Vec<DualAxisProcessor>,
}

impl UserInput for HorizontalAxis {
    fn kind(&self) -> InputControlKind {
        InputControlKind::DualAxis
    }

    fn decompose(&self) -> BasicInputs {
        BasicInputs::Composite(vec![self.negative.clone(), self.positive.clone()])
    }
}

#[serde_typetag]
impl DualAxislike for HorizontalAxis {
    fn get_axis_pair(&self, input_store: &CentralInputStore, gamepad: Entity) -> Option<Vec2> {
        let negative = self.negative.get_value(input_store, gamepad);
        let positive = self.positive.get_value(input_store, gamepad);
        if negative.is_none() && positive.is_none() {
            return None;
        }

        let value = Vec2::new(positive.unwrap_or(0.0) - negative.unwrap_or(0.0), 0.0);
        Some(
            self.processors
                .iter()
                .fold(value, |value, processor| processor.process(value)),
        )
    }

    fn set_axis_pair_as_gamepad(&self, world: &mut World, value: Vec2, gamepad: Option<Entity>) {
        if value.x < 0.0 {
            self.negative
                .set_value_as_gamepad(world, value.x.abs(), gamepad);
        } else if value.x > 0.0 {
            self.positive.set_value_as_gamepad(world, value.x, gamepad);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::input::InputPlugin;
    use leafwing_input_manager::plugin::CentralInputStorePlugin;
    use leafwing_input_manager::user_input::testing_utils::FetchUserInput;

    fn axis() -> HorizontalAxis {
        HorizontalAxis {
            negative: Box::new(KeyCode::KeyQ),
            positive: Box::new(KeyCode::KeyE),
            processors: Vec::new(),
        }
    }

    fn read_axis(key: KeyCode) -> Vec2 {
        let mut app = App::new();
        app.add_plugins((InputPlugin, CentralInputStorePlugin));
        key.press(app.world_mut());
        app.update();
        app.read_dual_axis_values(axis())
    }

    #[test]
    fn negative_and_positive_buttons_move_the_axis_only() {
        assert_eq!(read_axis(KeyCode::KeyQ), Vec2::new(-1.0, 0.0));
        assert_eq!(read_axis(KeyCode::KeyE), Vec2::new(1.0, 0.0));
        assert_eq!(read_axis(KeyCode::KeyW), Vec2::ZERO);
    }
}
