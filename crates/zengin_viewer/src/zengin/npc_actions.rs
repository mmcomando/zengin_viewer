//! FIFO of npc actions. The front action is executed every frame until it
//! reports completion; an empty queue leaves the npc standing still.

use std::collections::VecDeque;

use avian3d::prelude::SpatialQuery;
use bevy::prelude::*;

use crate::zengin::instance_index::InstanceIndex;
use crate::zengin::npc_routines::{NPC_BODY_Y_OFFSET, NpcPath, NpcRoutineState, WayPoints};
use crate::zengin::script::component::ScriptResource;
use crate::zengin::script::state::{AlignToWpChange, GotoChange};
use crate::zengin::script::string_value::StringCanon;
use crate::zengin::script::symbol_id::StringId;
use crate::zengin::waypoint_graph::{WaypointGraph, WaypointNodeIndex, nearest_waypoint_node};

/// Upper bound of queued actions, including the action at the front.
const MAX_QUEUED_ACTIONS: usize = 8;

/// Distance in Bevy meters within which an npc counts as standing on the target
/// waypoint and only has to align.
const ARRIVE_DISTANCE: f32 = 0.5;

/// The action at the front is executed every frame until it is done.
#[derive(Component, Default)]
pub(crate) struct NpcActionQueue {
    actions: VecDeque<NpcAction>,
    /// Front action has already been started (its [`NpcPath`] was inserted).
    started: bool,
}

impl NpcActionQueue {
    /// Appends an action, `false` when the queue is full. A `GoToWaypoint`
    /// replaces the `GoToWaypoint` at the back instead of stacking on it.
    pub(crate) fn push(&mut self, action: NpcAction) -> bool {
        match action {
            NpcAction::GoToWaypoint { waypoint } => self.push_goto(waypoint),
        }
    }

    fn push_goto(&mut self, waypoint: StringCanon) -> bool {
        if let Some(NpcAction::GoToWaypoint { waypoint: last }) = self.actions.back_mut() {
            *last = waypoint;
            // The replaced action was the active front, its path must be replanned.
            if self.actions.len() == 1 {
                self.started = false;
            }
            return true;
        }
        if self.actions.len() >= MAX_QUEUED_ACTIONS {
            return false;
        }
        self.actions.push_back(NpcAction::GoToWaypoint { waypoint });
        true
    }

    fn front(&self) -> Option<&NpcAction> {
        self.actions.front()
    }

    fn pop_front(&mut self) {
        self.actions.pop_front();
        self.started = false;
    }

    fn started(&self) -> bool {
        self.started
    }

    fn mark_started(&mut self) {
        self.started = true;
    }
}

#[derive(Clone, Debug)]
pub(crate) enum NpcAction {
    GoToWaypoint { waypoint: StringCanon },
}

/// Turns `AI_GoToWP` requests into queued actions. The waypoint name is resolved
/// through the script vm here because a host import cannot safely call into the
/// guest while it is running.
#[allow(clippy::cognitive_complexity)]
pub(crate) fn enqueue_goto_actions(
    mut changes: MessageReader<GotoChange>,
    index: Res<InstanceIndex>,
    gothic_script: Res<ScriptResource>,
    mut action_queues: Query<&mut NpcActionQueue>,
) {
    if changes.is_empty() {
        return;
    }

    let mut script = gothic_script.lock();
    for change in changes.read() {
        // String id 0 is unset; scripts pass it when there is no waypoint.
        if change.waypoint == 0 {
            continue;
        }
        let Some(string_id) = StringId::new(change.waypoint) else {
            warn!(
                "AI_GoToWP with invalid waypoint string id({}), npc({})",
                change.waypoint, change.instance_id
            );
            continue;
        };
        let Some(waypoint) = script.get_canon_string(string_id) else {
            warn!(
                "AI_GoToWP with unknown waypoint string id({}), npc({})",
                change.waypoint, change.instance_id
            );
            continue;
        };

        let mut queued = false;
        for &entity in index.npcs(change.instance_id) {
            let Ok(mut queue) = action_queues.get_mut(entity) else {
                continue;
            };
            queued |= queue.push(NpcAction::GoToWaypoint {
                waypoint: waypoint.clone(),
            });
        }
        if !queued {
            warn!(
                "AI_GoToWP waypoint({}) was not queued, npc({}) has no action queue or it is full",
                waypoint, change.instance_id
            );
        }
    }
}

/// Turns `AI_AlignToWP` requests into queued actions targeting the npc's
/// assigned routine waypoint. The action completes without walking when the npc
/// already stands on the waypoint.
pub(crate) fn enqueue_align_actions(
    mut changes: MessageReader<AlignToWpChange>,
    index: Res<InstanceIndex>,
    mut npcs: Query<(&NpcRoutineState, &mut NpcActionQueue)>,
) {
    for change in changes.read() {
        for &entity in index.npcs(change.instance_id) {
            let Ok((routine, mut queue)) = npcs.get_mut(entity) else {
                continue;
            };
            if !queue.push(NpcAction::GoToWaypoint {
                waypoint: routine.assigned_waypoint().clone(),
            }) {
                warn!(
                    "AI_AlignToWP waypoint({}) was not queued, npc({}) action queue is full",
                    routine.assigned_waypoint(),
                    change.instance_id
                );
            }
        }
    }
}

/// Executes the action at the front of every queue.
#[allow(clippy::type_complexity)]
pub(crate) fn run_npc_actions(
    mut commands: Commands,
    way_points: Res<WayPoints>,
    graph: Res<WaypointGraph>,
    spatial_query: SpatialQuery,
    waypoint_nodes: Query<&WaypointNodeIndex>,
    mut npcs: Query<(
        Entity,
        &mut NpcActionQueue,
        &mut Transform,
        Option<&NpcPath>,
    )>,
) {
    for (entity, mut queue, mut transform, path) in &mut npcs {
        let Some(action) = queue.front().cloned() else {
            continue;
        };
        let NpcAction::GoToWaypoint { waypoint } = action;

        if !queue.started() {
            // A replaced front action may still carry the previous path.
            commands.entity(entity).remove::<NpcPath>();
            start_goto(
                entity,
                &waypoint,
                &mut queue,
                &mut transform,
                &mut commands,
                &way_points,
                &graph,
                &spatial_query,
                &waypoint_nodes,
            );
            continue;
        }

        if path.is_some() {
            continue;
        }

        // `move_along_path` removed the path, so the npc arrived
        if let Some(waypoint_tr) = way_points.points.get(waypoint.as_str()) {
            transform.rotation = waypoint_tr.rotation;
        }
        debug!("npc({entity}) finished action goto({waypoint})");
        queue.pop_front();
    }
}

#[allow(clippy::too_many_arguments, clippy::cognitive_complexity)]
fn start_goto(
    entity: Entity,
    waypoint: &StringCanon,
    queue: &mut NpcActionQueue,
    transform: &mut Transform,
    commands: &mut Commands,
    way_points: &WayPoints,
    graph: &WaypointGraph,
    spatial_query: &SpatialQuery,
    waypoint_nodes: &Query<&WaypointNodeIndex>,
) {
    let Some(waypoint_tr) = way_points.points.get(waypoint.as_str()) else {
        warn!("npc({entity}) action goto({waypoint}), waypoint not found");
        queue.pop_front();
        return;
    };

    let target_position = waypoint_tr.translation + Vec3::Y * NPC_BODY_Y_OFFSET;
    if transform.translation.distance(target_position) <= ARRIVE_DISTANCE {
        transform.rotation = waypoint_tr.rotation;
        debug!("npc({entity}) finished action goto({waypoint}) without walking");
        queue.pop_front();
        return;
    }

    let path = nearest_waypoint_node(spatial_query, waypoint_nodes, graph, transform.translation)
        .and_then(|start| {
            let target = graph.node_by_name(waypoint.as_str())?;
            graph.shortest_path(start, target)
        });
    let Some(path) = path else {
        warn!("npc({entity}) action goto({waypoint}), no path, teleporting");
        teleport(transform, waypoint_tr);
        queue.pop_front();
        return;
    };

    debug!("npc({entity}) starting action goto({waypoint})");
    commands.entity(entity).insert(NpcPath::new(path));
    queue.mark_started();
}

fn teleport(transform: &mut Transform, waypoint: &Transform) {
    let scale = transform.scale;
    transform.translation = waypoint.translation + Vec3::Y * NPC_BODY_Y_OFFSET;
    transform.rotation = waypoint.rotation;
    transform.scale = scale;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn waypoint(name: &str) -> NpcAction {
        NpcAction::GoToWaypoint {
            waypoint: StringCanon::new(name),
        }
    }

    #[test]
    fn queue_replaces_the_last_goto() {
        let mut queue = NpcActionQueue::default();
        assert!(queue.push(waypoint("wp0")));
        assert!(queue.push(waypoint("wp1")));

        assert_eq!(queue.actions.len(), 1);
        let Some(NpcAction::GoToWaypoint { waypoint }) = queue.front() else {
            panic!("front action missing");
        };
        assert_eq!(waypoint.as_str(), "wp1");
    }

    #[test]
    fn replacing_the_active_goto_restarts_it() {
        let mut queue = NpcActionQueue::default();
        queue.push(waypoint("wp0"));
        queue.mark_started();

        queue.push(waypoint("wp1"));

        assert!(!queue.started());
        let Some(NpcAction::GoToWaypoint { waypoint }) = queue.front() else {
            panic!("front action missing");
        };
        assert_eq!(waypoint.as_str(), "wp1");
    }
}
