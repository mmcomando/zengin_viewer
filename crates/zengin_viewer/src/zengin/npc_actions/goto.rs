use avian3d::prelude::SpatialQuery;
use bevy::prelude::*;

use crate::zengin::npc_actions::queue::NpcActionQueue;
use crate::zengin::npc_routines::{NPC_BODY_Y_OFFSET, NpcPath, WayPoints};
use crate::zengin::script::string_value::StringCanon;
use crate::zengin::waypoint_graph::{WaypointGraph, WaypointNodeIndex, nearest_waypoint_node};

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
        warn!("npc({entity}) action goto({waypoint}), no path, teleporting");
        teleport(transform, waypoint_tr);
        queue.pop_front();
        return;
    };

    debug!("npc({entity}) starting action goto({waypoint})");
    commands.entity(entity).insert(NpcPath::new(path));
    queue.mark_started();
}

/// Distance in Bevy meters within which an npc counts as standing on the target
/// waypoint and only has to align.
const ARRIVE_DISTANCE: f32 = 0.5;

fn teleport(transform: &mut Transform, waypoint: &Transform) {
    let scale = transform.scale;
    transform.translation = waypoint.translation + Vec3::Y * NPC_BODY_Y_OFFSET;
    transform.rotation = waypoint.rotation;
    transform.scale = scale;
}
