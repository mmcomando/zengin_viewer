use bevy::ecs::world::EntityWorldMut;
use bevy::prelude::*;

use crate::zengin::npc_actions::attack::AttackAction;
use crate::zengin::npc_actions::goto::GoToWaypointAction;
use crate::zengin::npc_actions::item_state::NpcItemUse;
use crate::zengin::npc_actions::item_state::UseItemStateAction;
use crate::zengin::npc_actions::stand_up::StandUpAction;
use crate::zengin::npc_actions::use_mob::UseMobAction;

/// The npc currently executes an action. Removed to cancel the action.
#[derive(Component, Default)]
pub(crate) struct ExecutingActionMarker;

/// Drops an item pose when an action that cannot hold one starts.
pub(crate) fn drop_item_pose_for_action(
    mut commands: Commands,
    npcs: Query<
        Entity,
        (
            Added<ExecutingActionMarker>,
            Or<(With<AttackAction>, With<UseMobAction>, With<StandUpAction>)>,
            With<NpcItemUse>,
        ),
    >,
) {
    for entity in &npcs {
        debug!("entity({entity}) action replaces the item pose");
        commands.entity(entity).remove::<NpcItemUse>();
    }
}

/// Removes every action component when the marker is removed.
pub(crate) fn remove_actions_on_marker_removed(
    removed: On<Remove<ExecutingActionMarker>>,
    mut commands: Commands,
) {
    commands
        .entity(removed.entity)
        .queue_silenced(|mut entity: EntityWorldMut| {
            entity.remove::<(
                GoToWaypointAction,
                AttackAction,
                UseMobAction,
                UseItemStateAction,
                StandUpAction,
            )>();
        });
}

/// Reports an action component removed while the marker is still present.
///
/// The marker owns the action lifetime; direct action removal leaves a stale
/// marker that blocks the queue.
pub(crate) fn warn_direct_action_removal<A: Component>(
    mut removed: RemovedComponents<A>,
    mut markers_removed: RemovedComponents<ExecutingActionMarker>,
    markers: Query<(), With<ExecutingActionMarker>>,
) {
    let cancelled: Vec<Entity> = markers_removed.read().collect();
    for entity in removed.read() {
        if cancelled.contains(&entity) {
            continue;
        }
        if markers.get(entity).is_err() {
            continue;
        }
        warn!(
            "action component {} removed directly from entity({entity}) while \
             ExecutingActionMarker is present; remove the marker instead",
            core::any::type_name::<A>()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zengin::script::string_value::StringCanon;

    fn world() -> World {
        let mut world = World::new();
        world.add_observer(remove_actions_on_marker_removed);
        world
    }

    #[test]
    fn removing_the_marker_removes_action_components() {
        let mut world = world();
        let entity = world
            .spawn(GoToWaypointAction {
                waypoint: StringCanon::new("wp0"),
            })
            .id();
        assert!(world.get::<ExecutingActionMarker>(entity).is_some());

        world.entity_mut(entity).remove::<ExecutingActionMarker>();
        world.flush();

        assert!(world.get::<GoToWaypointAction>(entity).is_none());
        assert!(world.get::<ExecutingActionMarker>(entity).is_none());
    }

    #[test]
    fn unrelated_components_survive_the_cascade() {
        let mut world = world();
        let entity = world
            .spawn((
                GoToWaypointAction {
                    waypoint: StringCanon::new("wp0"),
                },
                Transform::default(),
            ))
            .id();

        world.entity_mut(entity).remove::<ExecutingActionMarker>();
        world.flush();

        assert!(world.get::<GoToWaypointAction>(entity).is_none());
        assert!(world.get::<Transform>(entity).is_some());
    }
}
