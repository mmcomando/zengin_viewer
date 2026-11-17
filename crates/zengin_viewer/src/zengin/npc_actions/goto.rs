use avian3d::prelude::SpatialQuery;
use bevy::prelude::*;

use crate::zengin::npc_actions::action::ExecutingActionMarker;
use crate::zengin::npc_actions::queue::NpcActionQueue;
use crate::zengin::npc_routines::{NPC_BODY_Y_OFFSET, NpcPath, WayPoints};
use crate::zengin::script::string_value::StringCanon;
use crate::zengin::waypoint_graph::{WaypointGraph, WaypointNodeIndex, nearest_waypoint_node};

/// Walks the npc to a named waypoint.
#[derive(Component)]
#[require(ExecutingActionMarker)]
pub(crate) struct GoToWaypointAction {
    pub(crate) waypoint: StringCanon,
}

#[allow(clippy::too_many_arguments, clippy::cognitive_complexity)]
pub(super) fn start_goto(
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
        // A spot (freepoint) is not part of the way net: walk to the nearest
        // node and cover the short remaining distance directly.
        start_goto_spot(
            entity,
            queue,
            transform,
            commands,
            graph,
            spatial_query,
            waypoint_nodes,
            target_position,
            waypoint,
        );
        return;
    };

    debug!("npc({entity}) starting action goto({waypoint})");
    commands.entity(entity).insert(NpcPath::new(path));
    queue.mark_started();
}

#[allow(clippy::too_many_arguments)]
fn start_goto_spot(
    entity: Entity,
    queue: &mut NpcActionQueue,
    transform: &Transform,
    commands: &mut Commands,
    graph: &WaypointGraph,
    spatial_query: &SpatialQuery,
    waypoint_nodes: &Query<&WaypointNodeIndex>,
    target_position: Vec3,
    waypoint: &StringCanon,
) {
    let path = nearest_waypoint_node(spatial_query, waypoint_nodes, graph, transform.translation)
        .and_then(|start| graph.shortest_path(start, graph.nearest_node(target_position)?));
    let Some(path) = path else {
        warn!("npc({entity}) action goto spot({waypoint}), no path");
        queue.pop_front();
        return;
    };
    debug!("npc({entity}) starting action goto spot({waypoint})");
    commands
        .entity(entity)
        .insert(NpcPath::with_target(path, target_position));
    queue.mark_started();
}

/// Distance in Bevy meters within which an npc counts as standing on the target
/// waypoint and only has to align.
const ARRIVE_DISTANCE: f32 = 0.5;
