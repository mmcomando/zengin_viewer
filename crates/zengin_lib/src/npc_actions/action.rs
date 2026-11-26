use bevy::ecs::world::EntityWorldMut;
use bevy::prelude::*;

use crate::npc_actions::attack::AttackAction;
use crate::npc_actions::goto::GoToWaypointAction;
use crate::npc_actions::goto_npc::GoToNpcAction;
use crate::npc_actions::item_state::NpcItemUse;
use crate::npc_actions::item_state::UseItemStateAction;
use crate::npc_actions::stand_up::StandUpAction;
use crate::npc_actions::start_state::StartStateAction;
use crate::npc_actions::use_mob::UseMobAction;
use crate::npc_actions::wait::WaitAction;
use dev_util::perf_debug;

/// The npc currently executes an action. Removed to cancel the action.
#[derive(Component, Default)]
pub struct ExecutingActionMarker;

/// Drops an item pose when an action that cannot hold one starts.
#[allow(clippy::type_complexity)]
pub fn drop_item_pose_for_action(
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
    let _perf = perf_debug!();
    for entity in &npcs {
        debug!("entity({entity}) action replaces the item pose");
        commands.entity(entity).remove::<NpcItemUse>();
    }
}

/// Removes every action component when the marker is removed.
pub fn remove_actions_on_marker_removed(
    removed: On<Remove<ExecutingActionMarker>>,
    mut commands: Commands,
) {
    let _perf = perf_debug!();
    commands
        .entity(removed.entity)
        .queue_silenced(|mut entity: EntityWorldMut| {
            entity.remove::<(
                GoToWaypointAction,
                GoToNpcAction,
                AttackAction,
                UseMobAction,
                UseItemStateAction,
                WaitAction,
                StandUpAction,
                StartStateAction,
            )>();
        });
}

/// Reports an action component removed while the marker is still present.
///
/// The marker owns the action lifetime; direct action removal leaves a stale
/// marker that blocks the queue.
pub fn warn_direct_action_removal<A: Component>(
    mut removed: RemovedComponents<A>,
    mut markers_removed: RemovedComponents<ExecutingActionMarker>,
    markers: Query<(), With<ExecutingActionMarker>>,
) {
    let _perf = perf_debug!();
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
    use crate::npc_actions::item_state::{NpcItemUse, UseItemStateAction};
    use crate::npc_actions::stand_up::StandUpAction;
    use crate::npc_actions::use_mob::UseMobAction;
    use crate::script::string_value::StringCanon;
    use crate::script::symbol_id::InstanceId;

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

    #[test]
    fn removing_the_marker_removes_every_action_component() {
        let mut world = world();
        let item = InstanceId::new(1).expect("valid item instance id");
        let entity = world
            .spawn((
                GoToWaypointAction {
                    waypoint: StringCanon::new("wp0"),
                },
                AttackAction::new(Entity::PLACEHOLDER),
                UseMobAction::npc(StringCanon::new("BENCH"), 1),
                UseItemStateAction {
                    item,
                    scheme: StringCanon::new("LUTE"),
                    state: 1,
                },
                StandUpAction,
                NpcItemUse {
                    item,
                    scheme: StringCanon::new("LUTE"),
                    state: 0,
                    target: 1,
                    expected: None,
                },
            ))
            .id();

        world.entity_mut(entity).remove::<ExecutingActionMarker>();
        world.flush();

        assert!(world.get::<GoToWaypointAction>(entity).is_none());
        assert!(world.get::<AttackAction>(entity).is_none());
        assert!(world.get::<UseMobAction>(entity).is_none());
        assert!(world.get::<UseItemStateAction>(entity).is_none());
        assert!(world.get::<StandUpAction>(entity).is_none());
        // Effect components are not actions and stay.
        assert!(world.get::<NpcItemUse>(entity).is_some());
    }
}
