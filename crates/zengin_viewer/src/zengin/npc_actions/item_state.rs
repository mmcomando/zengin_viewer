//! Item-state animations requested through `AI_UseItemToState`.
//!
//! The script asks an npc to hold or use an item. The item's `scemename` is the
//! animation scheme; the npc enters it from the standing pose
//! (`T_<SCHEME>_STAND_2_S0`) and steps through the states towards the requested
//! one (`T_<SCHEME>_S<a>_2_S<b>`), settling on the looping pose
//! `S_<SCHEME>_S<target>`. A negative state releases the item
//! (`T_<SCHEME>_S<a>_2_STAND`) and restores the standing idle.

use bevy::prelude::*;

use crate::gameplay::objects::NpcModel;
use crate::graphics::{
    AnimationController, AnimationEnds, AnimationGraph, animation_name_from_path,
};
use crate::zengin::common::{animation_file_name, choose_animation_scheme};
use crate::zengin::npc_actions::action::ExecutingActionMarker;
use crate::zengin::npc_actions::queue::NpcActionQueue;
use crate::zengin::npc_routines::{
    AnimationFiles, NpcOverlay, queue_named_animation, queue_stand_animation,
};
use crate::zengin::resources::CachedAssetHandles;
use crate::zengin::script::component::ScriptResource;
use crate::zengin::script::state::BS_ITEMINTERACT;
use crate::zengin::script::string_value::StringCanon;
use crate::zengin::script::symbol_id::InstanceId;
use crate::zengin::{GameNpc, NpcMobUse};

/// Item state currently held by an npc.
#[derive(Component, Debug, Clone)]
pub(crate) struct NpcItemUse {
    /// Item instance the pose belongs to.
    pub item: InstanceId,
    /// Animation scheme stem of the item (`scemename`), lowercase.
    pub scheme: StringCanon,
    /// Pose the npc has reached.
    pub state: i32,
    /// Requested pose; negative while releasing.
    pub target: i32,
    /// Animation name (without scheme) the state machine last queued. Animation
    /// ends are only advanced when they match it, so unrelated scripted
    /// animations are ignored.
    pub expected: Option<String>,
}

/// Holds or releases an inventory item pose.
#[derive(Component)]
#[require(ExecutingActionMarker)]
pub(crate) struct UseItemStateAction {
    pub(crate) item: InstanceId,
    pub(crate) scheme: StringCanon,
    pub(crate) state: i32,
}

/// Executes the `AI_UseItemToState` action at the front of the queue.
#[allow(clippy::too_many_arguments)]
pub(super) fn run_item_use_state_action(
    commands: &mut Commands,
    entity: Entity,
    npc: &GameNpc,
    item: InstanceId,
    scheme: &StringCanon,
    state: i32,
    queue: &mut NpcActionQueue,
    mut item_use: Option<Mut<'_, NpcItemUse>>,
    controller: Option<Mut<AnimationController>>,
    model: &NpcModel,
    overlay: Option<&NpcOverlay>,
    moving: bool,
    busy_with_mob: bool,
    gothic_script: &ScriptResource,
    files: &AnimationFiles,
    graph: Option<&AnimationGraph>,
    handles: &mut CachedAssetHandles,
    asset_server: &AssetServer,
) {
    // The pose belongs to a standing npc; wait for the walk / mob use to finish.
    if moving || busy_with_mob {
        return;
    }
    let Some(mut controller) = controller else {
        crate::warn_once!(
            "npc({}) AI_UseItemToState({}) waits for its animation controller",
            npc.instance_id(),
            scheme
        );
        return;
    };

    if state < 0 {
        release_item_use(
            commands,
            entity,
            item_use.as_deref_mut(),
            &mut controller,
            model,
            overlay,
            files,
            handles,
            asset_server,
        );
        queue.pop_front();
        return;
    }

    if item_use.is_some() {
        // Already using this item: retarget the running state machine. Re-issuing
        // the same state from the routine loop is a no-op.
        if let Some(item_use) = item_use.as_deref_mut() {
            item_use.target = state;
        }
        queue.pop_front();
        return;
    }

    let present = gothic_script
        .lock()
        .inventory(npc.instance_id())
        .iter()
        .any(|(queued, _)| *queued == item);
    if !present {
        debug!(
            "AI_UseItemToState npc({}) item({}) missing from inventory, cancelling",
            npc.instance_id(),
            item.value()
        );
        queue.pop_front();
        return;
    }

    // Some item schemes (e.g. `MAPSEALED`) have no entry pose of their own; the
    // archive keeps their release under a sibling state (`MAP`). Resolve that
    // alias through the animation graph before giving up.
    let resolved = resolve_scheme(model, overlay, files, graph, scheme.as_str());
    let scheme = resolved.map_or_else(|| scheme.clone(), StringCanon::new);
    let expected = start_animation(
        &mut controller,
        model,
        overlay,
        files,
        handles,
        asset_server,
        scheme.as_str(),
    );
    commands.entity(entity).insert(NpcItemUse {
        item,
        scheme: scheme.clone(),
        state: 0,
        target: state,
        expected,
    });
    info!(
        "npc({}) starts item state scheme({}) item({}) target({})",
        npc.instance_id(),
        scheme,
        item.value(),
        state
    );
    queue.pop_front();
}

/// Starts the item pose and returns the animation name that was queued.
fn start_animation(
    controller: &mut AnimationController,
    model: &NpcModel,
    overlay: Option<&NpcOverlay>,
    files: &AnimationFiles,
    handles: &mut CachedAssetHandles,
    asset_server: &AssetServer,
    scheme: &str,
) -> Option<String> {
    let start = start_animation_name(scheme);
    if has_animation(model, overlay, files, &start) {
        queue(
            controller,
            model,
            overlay,
            files,
            handles,
            asset_server,
            &start,
        );
        return Some(start);
    }
    let pose = pose_animation_name(scheme, 0);
    if has_animation(model, overlay, files, &pose) {
        queue(
            controller,
            model,
            overlay,
            files,
            handles,
            asset_server,
            &pose,
        );
        return Some(pose);
    }
    debug!("AI_UseItemToState scheme({scheme}) has no start or state-0 animation");
    None
}

/// Animation scheme to use instead of `scheme` when it has no entry pose but its
/// release transition belongs to another state's pose.
fn resolve_scheme(
    model: &NpcModel,
    overlay: Option<&NpcOverlay>,
    files: &AnimationFiles,
    graph: Option<&AnimationGraph>,
    scheme: &str,
) -> Option<String> {
    if has_animation(model, overlay, files, &start_animation_name(scheme))
        || has_animation(model, overlay, files, &pose_animation_name(scheme, 0))
    {
        return None;
    }
    let graph = graph?;
    let base = model.animation_scheme.as_deref()?;
    let overlay_name = overlay.and_then(|overlay| overlay.0.as_deref());
    let release = release_animation_name(scheme, 0);
    let release_scheme = choose_animation_scheme(base, overlay_name, &release, &files.0);
    let node = format!("{release_scheme}-{release}");
    for predecessor in graph.predecessors(&node) {
        let Some((_, animation)) = predecessor.split_once('-') else {
            continue;
        };
        let Some(alias) = animation
            .strip_prefix("S_")
            .and_then(|state| state.strip_suffix("_S0"))
        else {
            continue;
        };
        if alias.eq_ignore_ascii_case(scheme) {
            continue;
        }
        let alias = alias.to_lowercase();
        if has_animation(model, overlay, files, &start_animation_name(&alias))
            || has_animation(model, overlay, files, &pose_animation_name(&alias, 0))
        {
            debug!("AI_UseItemToState scheme({scheme}) resolved to alias scheme({alias})");
            return Some(alias);
        }
    }
    None
}

/// Starts the release transition (or the standing idle when there is none) and
/// removes the component once the transition is done.
#[allow(clippy::too_many_arguments)]
fn release_item_use(
    commands: &mut Commands,
    entity: Entity,
    item_use: Option<&mut NpcItemUse>,
    controller: &mut AnimationController,
    model: &NpcModel,
    overlay: Option<&NpcOverlay>,
    files: &AnimationFiles,
    handles: &mut CachedAssetHandles,
    asset_server: &AssetServer,
) {
    let Some(item_use) = item_use else {
        return;
    };
    item_use.target = -1;
    let finished = item_use.expected.is_none()
        || continue_release(
            item_use,
            false,
            controller,
            model,
            overlay,
            files,
            handles,
            asset_server,
        );
    if finished {
        queue_stand_animation(controller, model, overlay, files, handles, asset_server);
        commands.entity(entity).remove::<NpcItemUse>();
    }
}

/// Queues the next step back towards the standing pose. Returns `true` when the
/// release is complete and the caller should restore the standing idle.
#[allow(clippy::too_many_arguments)]
fn continue_release(
    item_use: &mut NpcItemUse,
    ended_release: bool,
    controller: &mut AnimationController,
    model: &NpcModel,
    overlay: Option<&NpcOverlay>,
    files: &AnimationFiles,
    handles: &mut CachedAssetHandles,
    asset_server: &AssetServer,
) -> bool {
    if ended_release {
        return true;
    }
    let scheme = item_use.scheme.as_str();
    let release = release_animation_name(scheme, item_use.state);
    if has_animation(model, overlay, files, &release) {
        queue(
            controller,
            model,
            overlay,
            files,
            handles,
            asset_server,
            &release,
        );
        item_use.expected = Some(release);
        return false;
    }
    // Some schemes (instruments) have no `S<n>_2_STAND`; step down one state
    // first and try again from there, like the engine's pose solver.
    if item_use.state > 0 {
        let next = item_use.state - 1;
        let step = step_animation_name(scheme, item_use.state, next);
        if has_animation(model, overlay, files, &step) {
            queue(
                controller,
                model,
                overlay,
                files,
                handles,
                asset_server,
                &step,
            );
            item_use.state = next;
            item_use.expected = Some(step);
            return false;
        }
    }
    true
}

/// Advances the item-state machine when one of its animations ends.
pub(crate) fn advance_item_use_animations(
    mut commands: Commands,
    mut ends: MessageReader<AnimationEnds>,
    files: Res<AnimationFiles>,
    mut handles: ResMut<CachedAssetHandles>,
    asset_server: Res<AssetServer>,
    mut npcs: Query<(
        &mut AnimationController,
        &NpcModel,
        Option<&NpcOverlay>,
        &mut NpcItemUse,
    )>,
) {
    for end in ends.read() {
        let Ok((mut controller, model, overlay, mut item_use)) = npcs.get_mut(end.who) else {
            continue;
        };
        if controller.current() != &end.what_animation {
            continue;
        }
        let Some(path) = Handle::path(&end.what_animation) else {
            continue;
        };
        let Some(ended) = animation_name_from_path(path.path()) else {
            continue;
        };
        let Some(expected) = item_use.expected.as_deref() else {
            continue;
        };
        if ended != expected {
            // An unrelated scripted animation (random eat/drink variant) played;
            // restore the settled item pose afterwards.
            if item_use.target >= 0 && item_use.state == item_use.target {
                let pose = pose_animation_name(item_use.scheme.as_str(), item_use.state);
                queue(
                    &mut controller,
                    model,
                    overlay,
                    &files,
                    &mut handles,
                    &asset_server,
                    &pose,
                );
                item_use.expected = Some(pose);
            }
            continue;
        }

        if item_use.target < 0 {
            let release = release_animation_name(item_use.scheme.as_str(), item_use.state);
            let finished = continue_release(
                &mut item_use,
                ended == release,
                &mut controller,
                model,
                overlay,
                &files,
                &mut handles,
                &asset_server,
            );
            if finished {
                queue_stand_animation(
                    &mut controller,
                    model,
                    overlay,
                    &files,
                    &mut handles,
                    &asset_server,
                );
                commands.entity(end.who).remove::<NpcItemUse>();
                debug!(
                    "npc({}) item({}) released, standing idle",
                    end.who,
                    item_use.item.value()
                );
            }
            continue;
        }

        if item_use.state == item_use.target {
            let pose = pose_animation_name(item_use.scheme.as_str(), item_use.state);
            queue(
                &mut controller,
                model,
                overlay,
                &files,
                &mut handles,
                &asset_server,
                &pose,
            );
            item_use.expected = Some(pose);
            continue;
        }

        let next = if item_use.state < item_use.target {
            item_use.state + 1
        } else {
            item_use.state - 1
        };
        let step = step_animation_name(item_use.scheme.as_str(), item_use.state, next);
        let (animation, reached) = if has_animation(model, overlay, &files, &step) {
            (step, next)
        } else {
            let pose = pose_animation_name(item_use.scheme.as_str(), next);
            if has_animation(model, overlay, &files, &pose) {
                (pose, next)
            } else {
                // No animation for the next state (e.g. FIRESPIT state 5); stay
                // at the current pose instead of stalling.
                (
                    pose_animation_name(item_use.scheme.as_str(), item_use.state),
                    item_use.state,
                )
            }
        };
        queue(
            &mut controller,
            model,
            overlay,
            &files,
            &mut handles,
            &asset_server,
            &animation,
        );
        item_use.state = reached;
        item_use.expected = Some(animation);
    }
}

/// Mirrors the item pose onto the body state reported to the scripts.
pub(crate) fn sync_npc_item_body_state(
    gothic_script: Option<Res<ScriptResource>>,
    added: Query<(&GameNpc, &NpcItemUse), Added<NpcItemUse>>,
    mut removed: RemovedComponents<NpcItemUse>,
    npcs: Query<(&GameNpc, Option<&NpcMobUse>)>,
) {
    let Some(gothic_script) = gothic_script else {
        return;
    };
    let mut script = gothic_script.lock();
    for entity in removed.read() {
        let Ok((npc, mob_use)) = npcs.get(entity) else {
            continue;
        };
        // A remaining mob use keeps its own body state.
        if mob_use.is_some() {
            continue;
        }
        script.set_npc_body_state(npc.instance_id(), None);
    }
    for (npc, _) in &added {
        script.set_npc_body_state(npc.instance_id(), Some(BS_ITEMINTERACT));
        debug!(
            "npc({}) body state({BS_ITEMINTERACT}) for item use",
            npc.instance_id()
        );
    }
}

fn queue(
    controller: &mut AnimationController,
    model: &NpcModel,
    overlay: Option<&NpcOverlay>,
    files: &AnimationFiles,
    handles: &mut CachedAssetHandles,
    asset_server: &AssetServer,
    animation: &str,
) {
    queue_named_animation(
        controller,
        model,
        overlay,
        files,
        handles,
        asset_server,
        animation,
    );
}

fn has_animation(
    model: &NpcModel,
    overlay: Option<&NpcOverlay>,
    files: &AnimationFiles,
    animation: &str,
) -> bool {
    let Some(base) = model.animation_scheme.as_deref() else {
        return false;
    };
    let overlay = overlay.and_then(|overlay| overlay.0.as_deref());
    let scheme = choose_animation_scheme(base, overlay, animation, &files.0);
    files.0.contains(&animation_file_name(scheme, animation))
}

fn start_animation_name(scheme: &str) -> String {
    format!("T_{}_STAND_2_S0", scheme.to_uppercase())
}

fn pose_animation_name(scheme: &str, state: i32) -> String {
    format!("S_{}_S{state}", scheme.to_uppercase())
}

fn step_animation_name(scheme: &str, from: i32, to: i32) -> String {
    format!("T_{}_S{from}_2_S{to}", scheme.to_uppercase())
}

fn release_animation_name(scheme: &str, from: i32) -> String {
    format!("T_{}_S{from}_2_STAND", scheme.to_uppercase())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    /// `MAPSEALED` has only a release transition; its entry pose lives under the
    /// `MAP` state and must be recovered through the graph.
    #[test]
    fn sealed_map_resolves_to_the_map_scheme() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/anim_graph.ron");
        let graph = AnimationGraph::load_from_ron(&path).expect("animation graph ron");
        let files = AnimationFiles(
            graph
                .animation_names()
                .map(|name| format!("{name}.MAN"))
                .collect(),
        );
        let model = NpcModel {
            animation_scheme: Some("HUMANS".to_string()),
            ..Default::default()
        };

        assert_eq!(
            resolve_scheme(&model, None, &files, Some(&graph), "MAPSEALED"),
            Some("map".to_string())
        );
        // A scheme with its own entry pose is left untouched.
        assert_eq!(
            resolve_scheme(&model, None, &files, Some(&graph), "LUTE"),
            None
        );
    }
}
