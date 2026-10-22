//! Waypoint navigation graph built from the world way net.

use std::collections::HashMap;

use bevy::prelude::*;
use pathfinding::prelude::astar;
use zen_kit_rs::waynet::WayNet;

use crate::zengin::common::{get_world_dir_quat, get_world_pos};
use crate::zengin::loaders::ZenginVec3ToGlam;

/// Waypoint navigation graph built from the world way net (points + edges).
#[derive(Resource, Debug, Default)]
pub struct WaypointGraph {
    pub nodes: Vec<WaypointNode>,
    /// Lowercased waypoint name to node index
    pub name_to_index: HashMap<String, usize>,
    /// Adjacency list, neighbor index and edge length in Bevy meters
    pub adjacency: Vec<Vec<(usize, f32)>>,
}

#[derive(Debug)]
pub struct WaypointNode {
    // pub name: String,
    /// World position (Bevy units, `get_world_pos` conversion)
    pub position: Vec3,
    /// Gothic-space direction the waypoint faces
    // pub direction: Vec3,
    pub free: bool,
}

impl WaypointGraph {
    /// Nearest node to a world position. Linear scan, callers with many queries
    /// should cache the result.
    /// TODO add spatial query to get nearest node
    pub fn nearest_node(&self, position: Vec3) -> Option<usize> {
        self.nodes
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                a.position
                    .distance_squared(position)
                    .total_cmp(&b.position.distance_squared(position))
            })
            .map(|(index, _)| index)
    }

    /// Node index of a waypoint by name (case-insensitive).
    pub fn node_by_name(&self, name: &str) -> Option<usize> {
        self.name_to_index.get(&name.to_lowercase()).copied()
    }

    /// Shortest path between two nodes (inclusive), `None` when unreachable.
    /// A* over the way net adjacency, cost is the edge length in millimeters
    /// (`pathfinding` requires an ordered integer cost).
    pub fn shortest_path(&self, from: usize, to: usize) -> Option<Vec<usize>> {
        if from >= self.nodes.len() || to >= self.nodes.len() {
            return None;
        }
        let target = self.nodes[to].position;
        let cost = |meters: f32| (meters * 1000.0).round() as i64;
        astar(
            &from,
            |&node| {
                self.adjacency[node]
                    .iter()
                    .map(|&(next, meters)| (next, cost(meters)))
            },
            |&node| cost(self.nodes[node].position.distance(target)),
            |&node| node == to,
        )
        .map(|(path, _)| path)
    }
}

/// Spot - named location
#[derive(Resource, Debug, Default)]
pub struct Spots(pub HashMap<String, Transform>);

/// Builds the waypoint graph and the waypoint transforms from the world way
/// net. Waypoint positions are also keyed by their lowercased name for the
/// routines and `Npc_GetDistToWP`.
pub fn load_way_net_points(way_net: &WayNet) -> (WaypointGraph, HashMap<String, Transform>) {
    let points = way_net.points();
    let mut graph = WaypointGraph::default();
    let mut way_points = HashMap::new();
    for point in &points {
        let name = point.name().to_lowercase();
        let pos = get_world_pos(point.position().to_glam());
        let direction = point.direction();
        let dir_2d = Vec2 {
            x: direction.x,
            y: direction.z,
        };
        let rot = get_world_dir_quat(dir_2d);
        let tr = Transform::from_translation(pos).with_rotation(rot);
        way_points.insert(name.clone(), tr);

        let index = graph.nodes.len();
        graph.name_to_index.insert(name.clone(), index);
        graph.nodes.push(WaypointNode {
            // name,
            position: pos,
            // direction: Vec3::new(direction.x, direction.y, direction.z),
            free: point.free_point(),
        });
    }

    // Edge endpoints are copies of the point values, not the same objects, so
    // match by the (unique) lowercased waypoint name
    graph.adjacency = vec![Vec::new(); graph.nodes.len()];
    for edge in way_net.edges() {
        let start = graph
            .name_to_index
            .get(&edge.start_point().name().to_lowercase())
            .copied();
        let end = graph
            .name_to_index
            .get(&edge.end_point().name().to_lowercase())
            .copied();
        let (Some(start), Some(end)) = (start, end) else {
            continue;
        };
        let length = (graph.nodes[start].position - graph.nodes[end].position).length();
        graph.adjacency[start].push((end, length));
        graph.adjacency[end].push((start, length));
    }

    let edges = graph.adjacency.iter().map(Vec::len).sum::<usize>() / 2;
    info!(
        "Waypoint graph: {} nodes, {} edges",
        graph.nodes.len(),
        edges
    );
    (graph, way_points)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(position: Vec3) -> WaypointNode {
        WaypointNode {
            position,
            free: false,
        }
    }

    /// `a - b - c` (each edge 1m) plus a direct `a - c` shortcut (10m).
    fn graph() -> WaypointGraph {
        let mut name_to_index = HashMap::new();
        name_to_index.insert("a".to_string(), 0);
        name_to_index.insert("b".to_string(), 1);
        name_to_index.insert("c".to_string(), 2);
        WaypointGraph {
            nodes: vec![
                node(Vec3::new(0.0, 0.0, 0.0)),
                node(Vec3::new(1.0, 0.0, 0.0)),
                node(Vec3::new(2.0, 0.0, 0.0)),
            ],
            name_to_index,
            adjacency: vec![
                vec![(1, 1.0), (2, 10.0)],
                vec![(0, 1.0), (2, 1.0)],
                vec![(1, 1.0), (0, 10.0)],
            ],
        }
    }

    #[test]
    fn shortest_path_prefers_the_cheaper_route() {
        assert_eq!(graph().shortest_path(0, 2), Some(vec![0, 1, 2]));
    }

    #[test]
    fn nearest_node_finds_the_closest() {
        assert_eq!(graph().nearest_node(Vec3::new(1.9, 0.0, 0.0)), Some(2));
        assert_eq!(graph().node_by_name("B"), Some(1));
    }
}
