// Move to this to separate directory for helper binaries tests/examples or other suggest some solution, helper binaries should not be in form of tests, update justfile when moved
//! Animation graph generation from the compiled anims archive.
//!
//! Every `<SCHEME>-<NAME>.MAN` in `_WORK/DATA/ANIMS/_COMPILED` is classified:
//! `S_X` are state animations, `T_A_2_B` transitions from state `A` to state
//! `B`, `T_X` one-shots returning to their source state, `C_X_N` combos.
//! Which animation follows which comes from the compiled model script
//! (`<SCHEME>.MSB`): a sequence whose `next` chains to another animation
//! arrives there when it plays out.
//!
//! The result is a simple graph export — every animation with the names of
//! the animations that can follow it — written to a RON file. The runtime
//! [`AnimationGraph`](crate::graphics::animation::AnimationGraph)
//! loads that file; this module is only the generator, run through the
//! ignored test below.

// TODO is this needed this should be gates for with #[cfg(test)]
// Only the ignored generator test uses this module in test builds.
#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs::File;

use bevy::prelude::*;
use zen_kit_rs::model_scripts::ModelScript;
use zen_kit_rs::stream::Read;

use crate::zengin::loaders::vdf_reader::get_zengin_asset_bytes;

const COMPILED_ANIMS_DIR: &str = "/_WORK/DATA/ANIMS/_COMPILED";
/// Output path relative to the working directory (the workspace root, same
/// convention as the input profiles).
const OUTPUT_PATH: &str = "crates/zengin_viewer/assets/humans_anim_graph.ron";
/// Model scheme prefix collected into the graph (humans only for now).
const SCHEME_PREFIX: &str = "HUMANS";
/// The scheme every human overlay builds upon.
const HUMANS_BASE_SCHEME: &str = "HUMANS";

/// Per-scheme graph data loaded from the anims archive, keyed by scheme name
/// (e.g. `HUMANS`, `HUMANS_1HST1`).
#[derive(Default)]
pub struct AnimationGraphs {
    schemes: BTreeMap<String, SchemeGraph>,
}

/// State/transition data of one model scheme (an overlay scheme in terms of
/// `Mdl_ApplyOverlayMds`).
#[derive(Default)]
struct SchemeGraph {
    /// State animations, keys are state names without the `S_` prefix
    /// (e.g. `RUN`, `1HRUNL`).
    states: BTreeMap<String, AnimationEntry>,
    /// Transitions with the states they connect.
    transitions: Vec<TransitionEdge>,
    /// Model script aliases, animation name -> the animation it plays
    /// (e.g. `S_BOWWALK` -> `S_BOWRUN`).
    aliases: BTreeMap<String, String>,
    /// Combo chain animations (`C_X_N`), no graph semantics yet.
    combos: Vec<String>,
}

#[derive(Default)]
struct AnimationEntry {
    /// Full animation name as in the archive (e.g. `S_RUNL`).
    animation: String,
    /// Animation the model script chains after this state, when it is not a
    /// self loop.
    next: Option<String>,
}

struct TransitionEdge {
    from: String,
    to: Option<String>,
    /// Full animation name as in the archive (e.g. `T_RUN_2_RUNL`).
    animation: String,
    /// Animation the model script chains after this transition, e.g.
    /// `T_RUN_2_RUNL` -> `S_RUNL`. This is the state the transition really
    /// arrives in; it may differ from `to` (e.g. `T_BOWJUMPB` -> `S_BOWRUN`)
    /// or name a state without its own `.MAN` (e.g. `S_BOWAIM`).
    next: Option<String>,
}

/// `animation name -> next animation name` entries of one compiled model
/// script, uppercased. The script of a scheme describes every animation the
/// scheme declares, including the loop chaining.
type ScriptNextMap = HashMap<String, String>;

/// Animation names a script aliases to other animations, uppercased.
type ScriptAliasMap = HashMap<String, String>;

/// Builds the data of every `HUMANS*` scheme found in the anims archive.
fn build_human_graphs(vfs: &zen_kit_rs::vfs::Vfs) -> Option<AnimationGraphs> {
    let (scheme_files, schemes) = collect_schemes(vfs)?;
    let scripts = load_script_data(vfs, &schemes);

    let empty_script = ScriptData::default();
    let schemes = scheme_files
        .into_iter()
        .map(|(scheme, files)| {
            let script = scripts.get(&scheme).unwrap_or(&empty_script);
            (scheme, build_scheme_graph(&files, script))
        })
        .collect();
    Some(AnimationGraphs { schemes })
}

/// Groups the `HUMANS*` animation file names by their scheme. Returns the file
/// names per scheme (without the `.MAN` suffix, uppercased) and the set of
/// schemes.
fn collect_schemes(
    vfs: &zen_kit_rs::vfs::Vfs,
) -> Option<(BTreeMap<String, Vec<String>>, Vec<String>)> {
    let dir = vfs.resolve_path(COMPILED_ANIMS_DIR)?;
    let mut scheme_files: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for child in dir.enumerate_children() {
        let name = child.name().to_uppercase();
        let Some(stem) = name.strip_suffix(".MAN") else {
            continue;
        };
        let Some((scheme, animation)) = stem.split_once('-') else {
            continue;
        };
        if !scheme.starts_with(SCHEME_PREFIX) {
            continue;
        }
        scheme_files
            .entry(scheme.to_string())
            .or_default()
            .push(animation.to_string());
    }
    let schemes = scheme_files.keys().cloned().collect();
    Some((scheme_files, schemes))
}

/// Model script data of one scheme: the `animation -> next` chaining and the
/// alias table.
#[derive(Default)]
struct ScriptData {
    next: ScriptNextMap,
    aliases: ScriptAliasMap,
}

/// Loads the compiled model script of every scheme. Schemes without a script
/// simply have no chaining information; their transitions then fall back to
/// their declared target states.
fn load_script_data(vfs: &zen_kit_rs::vfs::Vfs, schemes: &[String]) -> HashMap<String, ScriptData> {
    let mut scripts = HashMap::new();
    for scheme in schemes {
        let path = format!("{COMPILED_ANIMS_DIR}/{scheme}.MSB");
        let Some(bytes) = get_zengin_asset_bytes(vfs, &path) else {
            warn!(
                "Model script({path}) not found, animations of scheme {scheme} have no chaining information"
            );
            continue;
        };
        let Some(script) = script_data(&bytes) else {
            warn!(
                "Model script({path}) failed to parse, animations of scheme {scheme} have no chaining information"
            );
            continue;
        };
        scripts.insert(scheme.clone(), script);
    }
    scripts
}

fn script_data(bytes: &[u8]) -> Option<ScriptData> {
    let read = Read::from_slice(bytes)?;
    let script = ModelScript::load(&read)?;
    let mut data = ScriptData::default();
    for animation in script.animations() {
        data.next.insert(
            animation.name().to_uppercase(),
            animation.next().to_uppercase(),
        );
    }
    for alias in script.animation_aliases() {
        data.aliases
            .insert(alias.name().to_uppercase(), alias.alias().to_uppercase());
    }
    Some(data)
}

/// Builds the state/transition data of one scheme from its animation names.
fn build_scheme_graph(files: &[String], script: &ScriptData) -> SchemeGraph {
    let mut graph = SchemeGraph {
        states: BTreeMap::new(),
        transitions: Vec::new(),
        aliases: script.aliases.clone().into_iter().collect(),
        combos: Vec::new(),
    };

    for name in files {
        if let Some(state) = name.strip_prefix("S_") {
            graph.states.insert(
                state.to_string(),
                AnimationEntry {
                    animation: name.clone(),
                    next: non_empty(script.next.get(name).cloned()),
                },
            );
        } else if let Some(rest) = name.strip_prefix("T_") {
            let (from, to) = match rest.split_once("_2_") {
                Some((from, to)) => (from.to_string(), Some(to.to_string())),
                // A one-shot without a target state returns to its source.
                None => (rest.to_string(), None),
            };
            graph.transitions.push(TransitionEdge {
                from,
                to,
                animation: name.clone(),
                next: non_empty(script.next.get(name).cloned()),
            });
        } else if name.starts_with("C_") {
            graph.combos.push(name.clone());
        } else {
            warn_once!(
                "Unknown animation kind({name}) in the anims archive, it is not added to the graph"
            );
        }
    }

    graph
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.is_empty())
}

/// Builds the simple graph export: every animation with the names of the
/// animations that can directly follow it. Successors come from the model
/// scripts (arrival chaining, loop self references) and from the transitions
/// declared for a state in any scheme of the model.
fn export_animation_graph(graphs: &AnimationGraphs) -> Vec<(String, Vec<String>)> {
    // State aliases are scheme scoped: an overlay may alias states
    // differently than the base scheme.
    let mut scheme_aliases: HashMap<&str, HashMap<String, String>> = HashMap::new();
    for (scheme, scheme_graph) in &graphs.schemes {
        let mut aliases = HashMap::new();
        for (name, target) in &scheme_graph.aliases {
            if let (Some(from), Some(to)) = (state_of(name), state_of(target)) {
                aliases.insert(from, to);
            }
        }
        scheme_aliases.insert(scheme.as_str(), aliases);
    }

    // Every animation file is a node, even without successors.
    let mut successors: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (scheme, scheme_graph) in &graphs.schemes {
        for state in scheme_graph.states.keys() {
            successors.entry(format!("{scheme}-S_{state}")).or_default();
        }
        for edge in &scheme_graph.transitions {
            successors
                .entry(format!("{scheme}-{}", edge.animation))
                .or_default();
        }
        for combo in &scheme_graph.combos {
            successors.entry(format!("{scheme}-{combo}")).or_default();
        }
    }

    // State animations: the script arrival chain plus every transition
    // declared for the state in any scheme.
    for (scheme, scheme_graph) in &graphs.schemes {
        for (state, entry) in &scheme_graph.states {
            let node = format!("{scheme}-S_{state}");
            let mut out: BTreeSet<String> = BTreeSet::new();
            if let Some(next) = &entry.next {
                if next == &entry.animation {
                    // A looping state chains to itself.
                    out.insert(node.clone());
                } else if let Some(arrival) =
                    arrival_node(&successors, &scheme_aliases, scheme, next)
                {
                    out.insert(arrival);
                }
            }
            let canonical = canonical_state(scheme, state, &scheme_aliases);
            for (other_scheme, other_graph) in &graphs.schemes {
                for edge in &other_graph.transitions {
                    if canonical_state(other_scheme, &edge.from, &scheme_aliases) != canonical {
                        continue;
                    }
                    out.insert(format!("{other_scheme}-{}", edge.animation));
                }
            }
            successors.get_mut(&node).unwrap().extend(out);
        }

        // Transitions: the state the model script chains after them.
        for edge in &scheme_graph.transitions {
            let node = format!("{scheme}-{}", edge.animation);
            let arrival = if edge.next.as_deref() == Some(edge.animation.as_str()) {
                // A looping transition chains to itself.
                Some(node.clone())
            } else if let Some(next) = &edge.next {
                arrival_node(&successors, &scheme_aliases, scheme, next)
            } else {
                let state = canonical_state(
                    scheme,
                    edge.to.as_ref().unwrap_or(&edge.from),
                    &scheme_aliases,
                );
                resolve_node(&successors, scheme, &format!("S_{state}"))
            };
            if let Some(arrival) = arrival {
                successors.get_mut(&node).unwrap().insert(arrival);
            }
        }
    }

    successors
        .into_iter()
        .map(|(name, set)| (name, set.into_iter().collect()))
        .collect()
}

/// Node name of an animation in the context of a scheme, falling back to the
/// base scheme when the scheme has no own file.
fn resolve_node(
    successors: &BTreeMap<String, BTreeSet<String>>,
    scheme: &str,
    animation: &str,
) -> Option<String> {
    let own = format!("{scheme}-{animation}");
    if successors.contains_key(&own) {
        return Some(own);
    }
    let base = format!("{HUMANS_BASE_SCHEME}-{animation}");
    if successors.contains_key(&base) {
        return Some(base);
    }
    None
}

/// Node the model script chains to after an animation: a state animation
/// resolves through its state (scheme file first, base scheme fallback),
/// any other name is taken literally.
fn arrival_node(
    successors: &BTreeMap<String, BTreeSet<String>>,
    scheme_aliases: &HashMap<&str, HashMap<String, String>>,
    scheme: &str,
    next: &str,
) -> Option<String> {
    let Some(state) = state_of(next) else {
        return resolve_node(successors, scheme, next);
    };
    let state = canonical_state(scheme, &state, scheme_aliases);
    match resolve_node(successors, scheme, &format!("S_{state}")) {
        Some(node) => Some(node),
        None => {
            warn_once!(
                "Arrival state({state}) of scheme {scheme} has no animation file, it is not reachable in the graph"
            );
            None
        }
    }
}

/// State name of an animation name (`S_RUN` -> `RUN`), `None` for other kinds.
fn state_of(animation: &str) -> Option<String> {
    animation.strip_prefix("S_").map(str::to_string)
}

/// Resolves a state name through the aliases of its scheme (falling back to
/// the base scheme's aliases); aim states canonicalize to their shoot pose
/// (`BOWAIM` and `BOWSHOOT` are the same graph state, the aim has no own
/// `.MAN`).
fn canonical_state(
    scheme: &str,
    state: &str,
    scheme_aliases: &HashMap<&str, HashMap<String, String>>,
) -> String {
    let state = scheme_aliases
        .get(scheme)
        .and_then(|aliases| aliases.get(state))
        .or_else(|| {
            scheme_aliases
                .get(HUMANS_BASE_SCHEME)
                .and_then(|aliases| aliases.get(state))
        })
        .cloned()
        .unwrap_or_else(|| state.to_string());
    match state.strip_suffix("AIM") {
        Some(prefix) => format!("{prefix}SHOOT"),
        None => state,
    }
}

fn save_graphs(export: &[(String, Vec<String>)]) {
    let mut config = ron::ser::PrettyConfig::default();
    config.depth_limit = 4;
    let contents = match ron::ser::to_string_pretty(export, config) {
        Ok(contents) => contents,
        Err(error) => {
            warn!("Failed to serialize the animation graph: {error}");
            return;
        }
    };
    let result = File::create(OUTPUT_PATH).and_then(|mut file| {
        use std::io::Write;
        file.write_all(contents.as_bytes())
    });
    match result {
        Ok(()) => {
            let edges: usize = export.iter().map(|(_, successors)| successors.len()).sum();
            info!(
                "Saved animation graph({OUTPUT_PATH}, animations {}, edges {edges})",
                export.len()
            );
        }
        Err(error) => {
            warn!("Failed to write {OUTPUT_PATH}: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regenerates `assets/humans_anim_graph.ron` from the Gothic 2 anims
    /// archives. The runtime animation graph only loads that file.
    #[test]
    #[ignore]
    fn save_animation_graph_ron() {
        let vfs = crate::zengin::loaders::vdf_reader::create_zengin_vfs();
        let vfs = vfs.lock().unwrap();
        let Some(graphs) = build_human_graphs(&vfs) else {
            panic!("Compiled anims directory({COMPILED_ANIMS_DIR}) not found");
        };
        save_graphs(&export_animation_graph(&graphs));
    }
}
