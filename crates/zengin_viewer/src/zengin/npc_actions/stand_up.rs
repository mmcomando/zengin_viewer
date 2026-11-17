use bevy::prelude::*;

use crate::zengin::npc_actions::action::ExecutingActionMarker;

/// Stops the npc and stands it up idle.
#[derive(Component)]
#[require(ExecutingActionMarker)]
pub(crate) struct StandUpAction;
