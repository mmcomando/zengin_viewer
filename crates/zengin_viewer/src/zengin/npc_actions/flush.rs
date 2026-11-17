use bevy::prelude::*;

use crate::graphics::AnimationController;
use crate::zengin::npc_actions::action::ExecutingActionMarker;
use crate::zengin::npc_actions::queue::{NpcActionQueue, QueuedAction};

/// Moves the front queued action onto an npc that is not executing anything.
pub(crate) fn flush_npc_action_queue(
    mut commands: Commands,
    mut npcs: Query<
        (Entity, &mut NpcActionQueue, Option<&AnimationController>),
        Without<ExecutingActionMarker>,
    >,
) {
    for (entity, mut queue, controller) in &mut npcs {
        if !queue.can_flush_front(controller.is_some()) {
            continue;
        }
        let Some(action) = queue.pop_front() else {
            continue;
        };
        match action {
            QueuedAction::GoTo(action) => commands.entity(entity).insert(action),
            QueuedAction::Attack(action) => commands.entity(entity).insert(action),
            QueuedAction::UseMob(action) => commands.entity(entity).insert(action),
            QueuedAction::UseItemState(action) => commands.entity(entity).insert(action),
            QueuedAction::StandUp(action) => commands.entity(entity).insert(action),
        };
    }
}
