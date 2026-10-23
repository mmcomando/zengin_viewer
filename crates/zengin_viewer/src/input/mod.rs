mod actions;
mod options;
mod record;
mod replay;

pub(crate) use crate::input::actions::{
    ActionsPlugin, DebugAction, DebugInputMarker, GameplayAction, GameplayInputMarker, GuiAction,
    GuiInputMarker, InputMode, SharedAction, SharedInputMarker,
};
pub(crate) use crate::input::options::{Options, OptionsPlugin, add_renderless_default_plugins};
pub(crate) use crate::input::record::CameraSwitchedToThirdPerson;
pub(crate) use crate::input::replay::apply_scenario_window_settings;
