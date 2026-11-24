use bevy::prelude::*;

use crate::graphics::AnimationController;
use crate::perf_debug;
use crate::zengin::NpcCorpseMarker;
use crate::zengin::npc_actions::action::ExecutingActionMarker;
use crate::zengin::npc_actions::queue::{NpcActionQueue, QueuedAction};

/// Moves the front queued action onto an npc that is not executing anything and
/// has an animation controller, i.e. is animated. Far npcs that were never near
/// keep their actions queued.
#[allow(clippy::type_complexity)]
pub(crate) fn flush_npc_action_queue(
    mut commands: Commands,
    mut npcs: Query<
        (Entity, &mut NpcActionQueue),
        (
            With<AnimationController>,
            Without<ExecutingActionMarker>,
            Without<NpcCorpseMarker>,
        ),
    >,
) {
    let _perf = perf_debug!();
    for (entity, mut queue) in &mut npcs {
        let Some(action) = queue.pop_front() else {
            continue;
        };
        match action {
            QueuedAction::GoTo(action) => commands.entity(entity).insert(action),
            QueuedAction::GoToNpc(action) => commands.entity(entity).insert(action),
            QueuedAction::Attack(action) => commands.entity(entity).insert(*action),
            QueuedAction::UseMob(action) => commands.entity(entity).insert(action),
            QueuedAction::UseItemState(action) => commands.entity(entity).insert(action),
            QueuedAction::Wait(action) => commands.entity(entity).insert(action),
            QueuedAction::StandUp(action) => commands.entity(entity).insert(action),
            QueuedAction::StartState(action) => commands.entity(entity).insert(action),
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
    fn nothing_flushes_without_an_animation_controller() {
        let mut app = app();
        let mut queue = NpcActionQueue::default();
        assert!(queue.push_goto(StringCanon::new("wp0")));
        let entity = app.world_mut().spawn(queue).id();

        app.update();

        assert!(app.world().get::<GoToWaypointAction>(entity).is_none());
        assert!(
            !app.world()
                .get::<NpcActionQueue>(entity)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn attack_flushes_once_the_animation_controller_appears() {
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
