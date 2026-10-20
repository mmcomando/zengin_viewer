//! Animation transition graph loaded from the exported RON file.
//!
//! The graph is fully data driven: nodes are animation names as in the anims
//! archive (e.g. `HUMANS-S_RUN`), and every node lists the names of the
//! animations that can directly follow it. There is no state abstraction and
//! no scheme logic here — the engine (resolver) decides which animation it
//! wants to play, and the graph finds a way to reach it.

use std::collections::{HashMap, VecDeque};
use std::path::Path;

use bevy::prelude::*;

/// Distinct index of one animation inside an [`AnimationGraph`].
// Used by tests and the upcoming sequencer integration.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AnimationIndex(u32);

/// Animation transition graph with index keys.
// Used by tests and the upcoming sequencer integration.
#[allow(dead_code)]
#[derive(Resource, Default)]
pub struct AnimationGraph {
    /// Index -> animation name.
    names: Vec<String>,
    indices: HashMap<String, AnimationIndex>,
    /// Successor animation indices per node.
    next: Vec<Vec<AnimationIndex>>,
}

// Used by tests and the upcoming sequencer integration.
#[allow(dead_code)]
impl AnimationGraph {
    /// Loads the graph from the exported RON file:
    /// `[(animation, [successor animations])]`.
    pub fn load_from_ron(path: &Path) -> Option<AnimationGraph> {
        Some(Self::from_export(read_ron_export(path)?))
    }

    /// Adds edges from an additional RON file in the same
    /// `[(animation, [successor animations])]` format. All referenced
    /// animations have to already exist in the graph — missing entries are
    /// skipped with a warning, no new nodes are created and already present
    /// edges are not duplicated.
    pub fn load_additional_edges(&mut self, path: &Path) {
        let Some(export) = read_ron_export(path) else {
            return;
        };
        for (name, successors) in export {
            let Some(index) = self.indices.get(&name).copied() else {
                warn!(
                    "Manual edge node({name}) missing from the animation graph({path:?}), it is skipped"
                );
                continue;
            };
            let list = &mut self.next[index.0 as usize];
            for successor in successors {
                let Some(successor) = self.indices.get(&successor).copied() else {
                    warn!(
                        "Manual edge successor({successor}) of {name} missing from the animation graph({path:?}), it is skipped"
                    );
                    continue;
                };
                if !list.contains(&successor) {
                    list.push(successor);
                }
            }
        }
    }

    /// Builds a graph from parsed export entries, interning all node names.
    fn from_export(export: Vec<(String, Vec<String>)>) -> AnimationGraph {
        let mut graph = AnimationGraph::default();
        // Intern all names first, so successor references resolve.
        for (name, _) in &export {
            graph.intern(name);
        }
        graph.next = (0..graph.names.len()).map(|_| Vec::new()).collect();
        for (name, successors) in export {
            let index = graph.index(&name);
            let mut list = Vec::new();
            for successor in successors {
                match graph.indices.get(&successor) {
                    Some(successor) => list.push(*successor),
                    None => warn_once!(
                        "Successor({successor}) of {name} missing from the animation graph, it is skipped"
                    ),
                }
            }
            graph.next[index.0 as usize] = list;
        }
        graph
    }

    fn intern(&mut self, name: &str) -> AnimationIndex {
        if let Some(index) = self.indices.get(name) {
            return *index;
        }
        let index = AnimationIndex(self.names.len() as u32);
        self.names.push(name.to_string());
        self.indices.insert(name.to_string(), index);
        index
    }

    fn index(&self, name: &str) -> AnimationIndex {
        self.indices
            .get(name)
            .copied()
            .unwrap_or_else(|| unreachable!("interned animation({name}) missing"))
    }

    /// Index of an animation by its full name.
    pub fn animation_index(&self, name: &str) -> Option<AnimationIndex> {
        self.indices.get(name).copied()
    }

    /// Name of an animation index.
    pub fn animation_name(&self, index: AnimationIndex) -> &str {
        &self.names[index.0 as usize]
    }

    /// Next animation to play to reach the `desired` animation from the
    /// `current` one. Both are animation indices (see
    /// [`Self::animation_index`]).
    ///
    /// Returns the first animation of the shortest path through the graph;
    /// playing the following animations (each transition ends in the
    /// animation it leads to) reaches the desired one. `None` means the
    /// desired animation is already playing or no transition path leads to
    /// it — the caller has to cut directly.
    pub fn next_animation(
        &self,
        current: AnimationIndex,
        desired: AnimationIndex,
    ) -> Option<AnimationIndex> {
        if current == desired {
            return None;
        }

        let mut came_from: HashMap<AnimationIndex, AnimationIndex> = HashMap::new();
        let mut queue = VecDeque::from([current]);
        while let Some(animation) = queue.pop_front() {
            for successor in &self.next[animation.0 as usize] {
                if *successor == desired {
                    // Walk the predecessor chain back to the first hop.
                    let mut first = *successor;
                    let mut step = animation;
                    while let Some(previous) = came_from.get(&step) {
                        first = step;
                        step = *previous;
                    }
                    return Some(first);
                }
                if came_from.contains_key(successor) || *successor == current {
                    continue;
                }
                came_from.insert(*successor, animation);
                queue.push_back(*successor);
            }
        }

        None
    }
}

/// Reads and parses a RON file with `[(animation, [successor animations])]`
/// entries. Returns `None` and logs on read or parse failure.
fn read_ron_export(path: &Path) -> Option<Vec<(String, Vec<String>)>> {
    let contents = match std::fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) => {
            warn!("Failed to read animation graph({path:?}): {error}");
            return None;
        }
    };
    match ron::de::from_str(&contents) {
        Ok(export) => Some(export),
        Err(error) => {
            warn!("Failed to parse animation graph({path:?}): {error}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Stand -> run in: from the standing idle the run-in transition is
    /// directly reachable.
    #[test]
    fn stand_to_run_in() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/humans_anim_graph.ron");
        let graph = AnimationGraph::load_from_ron(&path).expect("animation graph ron");

        let stand = graph
            .animation_index("HUMANS-S_RUN")
            .expect("standing idle in the graph");
        let run_in = graph
            .animation_index("HUMANS-T_RUN_2_RUNL")
            .expect("run-in transition in the graph");

        assert_eq!(graph.next_animation(stand, run_in), Some(run_in));
    }

    /// Additional edges only add successors of existing nodes, never
    /// duplicate entries and never new indices.
    #[test]
    fn additional_edges_merge() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/humans_anim_graph.ron");
        let mut graph = AnimationGraph::load_from_ron(&path).expect("animation graph ron");
        let before: Vec<_> = graph.names.clone();
        let manual =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/humans_anim_manual_edges.ron");
        graph.load_additional_edges(&manual);

        assert_eq!(graph.names, before, "no new nodes are interned");
        let runl = graph
            .animation_index("HUMANS-S_RUNL")
            .expect("run loop in the graph");
        let two_hand = graph
            .animation_index("HUMANS-S_2HRUNL")
            .expect("two hand run loop in the graph");
        let successors = &graph.next[runl.0 as usize];
        assert!(successors.contains(&two_hand), "manual edge added");
        assert_eq!(
            successors.len(),
            successors
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len(),
            "no duplicated edges"
        );
    }
}
