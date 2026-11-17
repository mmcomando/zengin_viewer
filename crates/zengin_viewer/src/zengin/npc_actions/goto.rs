use avian3d::prelude::SpatialQuery;
use bevy::prelude::*;

use crate::gameplay::objects::NpcModel;
use crate::graphics::{AnimationController, AutoAnimSequencer};
use crate::zengin::npc_actions::action::ExecutingActionMarker;
use crate::zengin::npc_routines::{
    AnimationFiles, NPC_BODY_Y_OFFSET, NpcOverlay, NpcPath, WayPoints, queue_stand_animation,
};
use crate::zengin::resources::CachedAssetHandles;
use crate::zengin::script::string_value::StringCanon;
use crate::zengin::waypoint_graph::{WaypointGraph, WaypointNodeIndex, nearest_waypoint_node};

/// Walks the npc to a named waypoint.
#[derive(Component)]
#[require(ExecutingActionMarker)]
pub(crate) struct GoToWaypointAction {
    pub(crate) waypoint: StringCanon,
}

/// Distance in Bevy meters within which an npc counts as standing on the target
/// waypoint and only has to align.
const ARRIVE_DISTANCE: f32 = 0.5;

#[allow(clippy::too_many_arguments)]
pub(crate) fn start_goto_action(
    mut commands: Commands,
    way_points: Res<WayPoints>,
    graph: Res<WaypointGraph>,
    spatial_query: SpatialQuery,
    waypoint_nodes: Query<&WaypointNodeIndex>,
    mut npcs: Query<(Entity, &GoToWaypointAction, &mut Transform), Added<GoToWaypointAction>>,
) {
    for (entity, action, mut transform) in &mut npcs {
        // A canceled goto may still carry the previous path.
        commands.entity(entity).remove::<NpcPath>();
        start_goto(
            entity,
            &action.waypoint,
            &mut transform,
            &mut commands,
            &way_points,
            &graph,
            &spatial_query,
            &waypoint_nodes,
        );
    }
}

pub(crate) fn run_goto_action(
    mut commands: Commands,
    way_points: Res<WayPoints>,
    mut npcs: Query<(
        Entity,
        &GoToWaypointAction,
        &mut Transform,
        Option<&NpcPath>,
    )>,
) {
    for (entity, action, mut transform, path) in &mut npcs {
        if path.is_some() {
            continue;
        }
        // `move_along_path` removed the path, so the npc arrived.
        if let Some(waypoint_tr) = way_points.points.get(action.waypoint.as_str()) {
            transform.rotation = waypoint_tr.rotation;
        }
        debug!("npc({entity}) finished action goto({})", action.waypoint);
        commands.entity(entity).remove::<ExecutingActionMarker>();
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn end_goto_action(
    mut commands: Commands,
    mut removed: RemovedComponents<GoToWaypointAction>,
    files: Res<AnimationFiles>,
    mut handles: ResMut<CachedAssetHandles>,
    asset_server: Res<AssetServer>,
    mut npcs: Query<(
        Option<&NpcPath>,
        Option<&mut AutoAnimSequencer>,
        Option<&mut AnimationController>,
        &NpcModel,
        Option<&NpcOverlay>,
    )>,
) {
    for entity in removed.read() {
        let Ok((path, mut sequencer, controller, model, overlay)) = npcs.get_mut(entity) else {
            continue;
        };
        // A path means `move_along_path` did not finish, the action was canceled.
        if path.is_none() {
            continue;
        }
        commands.entity(entity).remove::<NpcPath>();
        let Some(mut controller) = controller else {
            continue;
        };
        controller.set_root_motion(false);
        queue_stand_animation(
            &mut controller,
            sequencer.as_deref_mut(),
            model,
            overlay,
            &files,
            &mut handles,
            &asset_server,
        );
    }
}

#[allow(clippy::too_many_arguments, clippy::cognitive_complexity)]
fn start_goto(
    entity: Entity,
    waypoint: &StringCanon,
    transform: &mut Transform,
    commands: &mut Commands,
    way_points: &WayPoints,
    graph: &WaypointGraph,
    spatial_query: &SpatialQuery,
    waypoint_nodes: &Query<&WaypointNodeIndex>,
) {
    let Some(waypoint_tr) = way_points.points.get(waypoint.as_str()) else {
        warn!("npc({entity}) action goto({waypoint}), waypoint not found");
        commands.entity(entity).remove::<ExecutingActionMarker>();
        return;
    };

    let target_position = waypoint_tr.translation + Vec3::Y * NPC_BODY_Y_OFFSET;
    if transform.translation.distance(target_position) <= ARRIVE_DISTANCE {
        transform.rotation = waypoint_tr.rotation;
        debug!("npc({entity}) finished action goto({waypoint}) without walking");
        commands.entity(entity).remove::<ExecutingActionMarker>();
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
}

#[allow(clippy::too_many_arguments)]
fn start_goto_spot(
    entity: Entity,
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
        commands.entity(entity).remove::<ExecutingActionMarker>();
        return;
    };
    debug!("npc({entity}) starting action goto spot({waypoint})");
    commands
        .entity(entity)
        .insert(NpcPath::with_target(path, target_position));
}
