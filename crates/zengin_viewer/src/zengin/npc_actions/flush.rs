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
            QueuedAction::Attack(action) => commands.entity(entity).insert(*action),
            QueuedAction::UseMob(action) => commands.entity(entity).insert(action),
            QueuedAction::UseItemState(action) => commands.entity(entity).insert(action),
            QueuedAction::StandUp(action) => commands.entity(entity).insert(action),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphics::{AnimationController, AnimationData};
    use crate::zengin::npc_actions::action::ExecutingActionMarker;
    use crate::zengin::npc_actions::attack::AttackAction;
    use crate::zengin::npc_actions::goto::GoToWaypointAction;
    use crate::zengin::script::string_value::StringCanon;
    use bevy::asset::Handle;

    fn app() -> App {
        let mut app = App::new();
        app.add_systems(Update, flush_npc_action_queue);
        app
    }

    #[test]
    fn goto_flushes_without_a_controller() {
        let mut app = app();
        let mut queue = NpcActionQueue::default();
        assert!(queue.push_goto(StringCanon::new("wp0")));
        let entity = app.world_mut().spawn(queue).id();

        app.update();

        assert!(app.world().get::<GoToWaypointAction>(entity).is_some());
        assert!(app.world().get::<ExecutingActionMarker>(entity).is_some());
        assert!(
            app.world()
                .get::<NpcActionQueue>(entity)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn attack_waits_for_the_animation_controller() {
        let mut app = app();
        let mut queue = NpcActionQueue::default();
        assert!(queue.push_attack(Entity::PLACEHOLDER));
        let entity = app.world_mut().spawn(queue).id();

        app.update();
        assert!(app.world().get::<AttackAction>(entity).is_none());

        let controller = AnimationController::new(Handle::<AnimationData>::default());
        app.world_mut().entity_mut(entity).insert(controller);
        app.update();

        assert!(app.world().get::<AttackAction>(entity).is_some());
        assert!(app.world().get::<ExecutingActionMarker>(entity).is_some());
    }
}
