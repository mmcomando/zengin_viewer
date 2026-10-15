//! Waypoint navigation graph built from the world way net.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

use bevy::prelude::*;

use crate::gui_debug::toggle_visibility::show_gizmos;

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
    /// A* over the way net adjacency, cost is the edge length in Bevy meters.
    /// TODO check if there is a library for A*
    pub fn shortest_path(&self, from: usize, to: usize) -> Option<Vec<usize>> {
        if from >= self.nodes.len() || to >= self.nodes.len() {
            return None;
        }
        if from == to {
            return Some(vec![from]);
        }

        let mut cost_so_far = vec![f32::INFINITY; self.nodes.len()];
        let mut came_from = vec![usize::MAX; self.nodes.len()];
        let mut closed = vec![false; self.nodes.len()];
        cost_so_far[from] = 0.0;

        let mut open = BinaryHeap::new();
        open.push(MinCost::new(from, 0.0, &self.nodes, to));

        while let Some(MinCost { node, .. }) = open.pop() {
            if node == to {
                break;
            }
            if closed[node] {
                continue;
            }
            closed[node] = true;

            for &(next, weight) in &self.adjacency[node] {
                if closed[next] {
                    continue;
                }
                let candidate = cost_so_far[node] + weight;
                if candidate < cost_so_far[next] {
                    cost_so_far[next] = candidate;
                    came_from[next] = node;
                    open.push(MinCost::new(next, candidate, &self.nodes, to));
                }
            }
        }

        if came_from[to] == usize::MAX {
            return None;
        }
        let mut path = vec![to];
        let mut node = to;
        while node != from {
            node = came_from[node];
            if node == usize::MAX {
                return None;
            }
            path.push(node);
        }
        path.reverse();
        Some(path)
    }
}

/// Min-heap entry for A*, ordered by `f = cost + heuristic`.
struct MinCost {
    cost: f32,
    node: usize,
}

impl MinCost {
    fn new(node: usize, cost: f32, nodes: &[WaypointNode], target: usize) -> Self {
        let heuristic = nodes[node].position.distance(nodes[target].position);
        Self {
            cost: cost + heuristic,
            node,
        }
    }
}

impl PartialEq for MinCost {
    fn eq(&self, other: &Self) -> bool {
        self.cost == other.cost
    }
}
impl Eq for MinCost {}
impl PartialOrd for MinCost {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for MinCost {
    // Reversed so the smallest cost pops first
    fn cmp(&self, other: &Self) -> Ordering {
        other.cost.total_cmp(&self.cost)
    }
}

/// Draws the waypoint graph as debug gizmos when the "Show Gizmos" toggle is on.
pub struct WaypointDebugPlugin;

impl Plugin for WaypointDebugPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, draw_waypoints.run_if(show_gizmos));
    }
}

fn draw_waypoints(mut gizmos: Gizmos, graph: Option<Res<WaypointGraph>>) {
    let Some(graph) = graph else {
        return;
    };
    for node in &graph.nodes {
        let color = if node.free {
            Color::srgb(1.0, 0.6, 0.0)
        } else {
            Color::srgb(0.2, 0.7, 1.0)
        };
        gizmos.sphere(node.position, 0.3, color);
    }
    let edge_color = Color::srgb(0.4, 0.4, 0.4);
    for (index, neighbors) in graph.adjacency.iter().enumerate() {
        for &(neighbor, _) in neighbors {
            // Draw each undirected edge once
            if neighbor <= index {
                continue;
            }
            gizmos.line(
                graph.nodes[index].position,
                graph.nodes[neighbor].position,
                edge_color,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(name: &str, position: Vec3) -> WaypointNode {
        WaypointNode {
            name: name.to_string(),
            position,
            direction: Vec3::ZERO,
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
                node("a", Vec3::new(0.0, 0.0, 0.0)),
                node("b", Vec3::new(1.0, 0.0, 0.0)),
                node("c", Vec3::new(2.0, 0.0, 0.0)),
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
