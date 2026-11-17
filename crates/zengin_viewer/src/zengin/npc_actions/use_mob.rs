use bevy::prelude::*;

use crate::zengin::npc_actions::action::ExecutingActionMarker;
use crate::zengin::script::string_value::StringCanon;

/// Uses the nearest mob with the given `AI_UseMob` scheme.
#[derive(Component)]
#[require(ExecutingActionMarker)]
pub(crate) struct UseMobAction {
    pub(crate) scheme: StringCanon,
    pub(crate) state: i32,
}
