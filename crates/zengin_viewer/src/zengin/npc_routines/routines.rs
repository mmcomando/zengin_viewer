//! Waypoint routines: active routine entries, their waypoint targets, routine
//! state execution and `AI_GoToWP` path following.

use std::collections::HashMap;
use std::sync::PoisonError;

use bevy::prelude::*;

use crate::gameplay::game_time::GameTime;
use crate::gameplay::objects::{NpcModel, PlayerCharacter};
use crate::graphics::{
    AnimationController, AutoAnimSequencer, DesiredAnimation, SetDesiredAnimation,
};
use crate::zengin::GameNpc;
use crate::zengin::common::{choose_animation_scheme, face_direction_quat, get_animation_path};
use crate::zengin::instance_index::InstanceIndex;
use crate::zengin::npc_actions::{NpcAction, NpcActionQueue, NpcItemUse};
use crate::zengin::npc_routines::overlay::{AnimationFiles, NpcOverlay};
use crate::zengin::resources::CachedAssetHandles;
use crate::zengin::script::component::ScriptResource;
use crate::zengin::script::state::{
    RoutineEntry, RoutineFunctions, ScriptFreePoint, StartStateChange, TurnToNpcChange,
};
use crate::zengin::script::string_value::StringCanon;
use crate::zengin::waypoint_graph::{Spots, WaypointGraph};
use crate::zengin::{DialogUiState, InstanceId};

/// Waypoint transforms of the loaded world. Spots take precedence over way points,
/// same lookup order as the initial npc placement.
#[derive(Resource, Default)]
pub struct WayPoints {
    pub points: HashMap<String, Transform>,
}

/// Matches the body offset applied when npcs are placed at world load.
pub(crate) const NPC_BODY_Y_OFFSET: f32 = -0.5;

#[derive(Component)]
pub(crate) struct NpcRoutineState {
    /// Waypoint the npc was last assigned to by a routine check
    assigned_waypoint: StringCanon,
}

impl NpcRoutineState {
    pub(crate) fn assigned_waypoint(&self) -> &StringCanon {
        &self.assigned_waypoint
    }
}

/// Engine loop cadence of a running routine state (`ZS_*_loop`), in seconds.
const ROUTINE_LOOP_INTERVAL_SECS: f32 = 1.0;

/// Why a routine state is running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RoutineSource {
    /// Started from the active [`RoutineEntry`].
    Routine,
    /// Pushed by `AI_StartState`; routine entry changes do not replace it.
    AiState,
}

/// Running routine state of an npc with its resolved start/loop/end hooks. The
/// lifecycle system starts the state, calls its loop once per
/// [`ROUTINE_LOOP_INTERVAL_SECS`] and runs its end hook when the state is
/// replaced or the component is removed.
#[derive(Component, Debug, Clone)]
pub(crate) struct RoutineExecutor {
    functions: RoutineFunctions,
    source: RoutineSource,
    /// Routine waypoint the state was started for, empty for AI states. A
    /// changed waypoint restarts the state even when the state function is the
    /// same, so its end hook releases the previous mob or path.
    way_point: StringCanon,
    /// Seconds since the state started or was last reset by `NPC_SetStateTime`.
    state_time: f32,
    /// Seconds until the next loop hook call.
    next_loop_in: f32,
}

#[derive(Resource, Default)]
pub(crate) struct NpcRoutineCheckState {
    last_checked_minute: Option<u32>,
}

/// Refreshes the active routine entry of already activated npcs on every in-game
/// minute change. Only npcs that already carry a [`RoutineEntry`] component are
/// touched; adding and removing the component is driven by the script bridge.
pub(crate) fn update_npc_routine_entries(
    mut check_state: ResMut<NpcRoutineCheckState>,
    game_time: Res<GameTime>,
    mut npcs: Query<(&GameNpc, &mut RoutineEntry), Without<ChildOf>>,
) {
    let minutes = game_time.minutes_of_day();
    if check_state.last_checked_minute == Some(minutes) {
        return;
    }
    check_state.last_checked_minute = Some(minutes);

    for (npc, mut entry) in &mut npcs {
        let Some(active) = npc
            .handle()
            .runtime()
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .get_routine_entry(minutes)
            .cloned()
        else {
            continue;
        };
        if *entry == active {
            continue;
        }
        *entry = active;
    }
}

/// Reacts to the [`RoutineEntry`] component of an npc being added or changed.
/// When the active routine entry changes, a waypoint action is queued.
#[allow(clippy::type_complexity)]
pub(crate) fn update_npc_routines(
    mut commands: Commands,
    dialog_ui: Res<DialogUiState>,
    mut npcs: Query<
        (
            Entity,
            &GameNpc,
            &RoutineEntry,
            Option<&mut NpcRoutineState>,
            Option<&mut NpcActionQueue>,
        ),
        (Changed<RoutineEntry>, Without<ChildOf>),
    >,
    mut removed: RemovedComponents<RoutineEntry>,
) {
    let talking_npc = dialog_ui.talking_npc();
    for entity in removed.read() {
        // Despawned entities are fine here (e.g. a killed npc), `try_remove`
        // treats the command as a no-op for them.
        commands.entity(entity).try_remove::<NpcRoutineState>();
    }

    for (entity, npc, entry, routine_state, mut queue) in &mut npcs {
        let Some(mut state) = routine_state else {
            // First assignment: npcs were already placed at the start hour by the
            // world load, only record the current assignment.
            commands.entity(entity).insert(NpcRoutineState {
                assigned_waypoint: entry.way_point.clone(),
            });
            continue;
        };
        if state.assigned_waypoint == entry.way_point {
            continue;
        }
        state.assigned_waypoint = entry.way_point.clone();
        queue_routine_waypoint(
            npc,
            entry,
            queue.as_deref_mut(),
            talking_npc == Some(npc.instance_id()),
        );
    }
}

/// Queues the waypoint of a changed routine entry, holding it while the npc is
/// talking or has another action pending. The routine loop re-issues the
/// movement once the npc is free.
fn queue_routine_waypoint(
    npc: &GameNpc,
    entry: &RoutineEntry,
    queue: Option<&mut NpcActionQueue>,
    talking: bool,
) {
    if talking {
        debug!(
            "talking routine npc({}) waypoint({}) not queued",
            npc.instance_id(),
            entry.way_point
        );
        return;
    }
    let Some(queue) = queue else {
        crate::warn_once!(
            "routine npc({}) has no action queue, waypoint({})",
            npc.instance_id(),
            entry.way_point
        );
        return;
    };
    if !queue.is_empty() {
        debug!(
            "routine npc({}) busy, waypoint({}) not queued",
            npc.instance_id(),
            entry.way_point
        );
        return;
    }
    if !queue.push(NpcAction::GoToWaypoint {
        waypoint: entry.way_point.clone(),
    }) {
        warn!(
            "routine npc({}) action queue full, waypoint({}) not queued",
            npc.instance_id(),
            entry.way_point
        );
    }
}

/// Starts and ends the talk pause: when a npc starts talking its queued actions
/// are dropped so it stops walking or using a mob, and when the dialog closes
/// its routine entry is marked changed so [`update_routine_executors`]
/// reconciles a schedule that moved on while the npc was talking.
pub(crate) fn pause_talking_npc_routine(
    dialog_ui: Res<DialogUiState>,
    index: Res<InstanceIndex>,
    mut last_talking: Local<Option<InstanceId>>,
    mut queues: Query<&mut NpcActionQueue>,
    mut entries: Query<&mut RoutineEntry>,
) {
    let talking = dialog_ui.talking_npc();
    if *last_talking == talking {
        return;
    }
    let previous = std::mem::replace(&mut *last_talking, talking);
    if let Some(npc) = talking {
        for &entity in index.npcs(npc) {
            if let Ok(mut queue) = queues.get_mut(entity) {
                queue.clear();
            }
        }
    }
    let Some(npc) = previous.filter(|npc| Some(*npc) != talking) else {
        return;
    };
    for &entity in index.npcs(npc) {
        if let Ok(mut entry) = entries.get_mut(entity) {
            entry.set_changed();
        }
    }
}

/// Runs the end hook of routine states whose [`RoutineEntry`] disappeared and
/// drops their executor.
pub(crate) fn end_removed_routine_entries(
    mut commands: Commands,
    gothic_script: Res<ScriptResource>,
    mut removed: RemovedComponents<RoutineEntry>,
    npcs: Query<(Entity, &GameNpc, &RoutineExecutor), Without<RoutineEntry>>,
) {
    let mut script = gothic_script.lock();
    for entity in removed.read() {
        // Despawned entities are fine here, the executor goes away with them.
        let Ok((entity, npc, executor)) = npcs.get(entity) else {
            continue;
        };
        if executor.source == RoutineSource::AiState {
            continue;
        }
        script.call_routine_end(npc.instance_id(), executor.functions);
        commands.entity(entity).remove::<RoutineExecutor>();
    }
}

/// Starts, replaces and removes the running routine state from the active
/// [`RoutineEntry`]. The previous state's end hook runs before the next start.
/// Executors are only created for npcs whose visual is spawned, far npcs run
/// neither their state script nor its animation.
#[allow(clippy::type_complexity)]
pub(crate) fn update_routine_executors(
    mut commands: Commands,
    gothic_script: Res<ScriptResource>,
    dialog_ui: Res<DialogUiState>,
    mut npcs: Query<
        (
            Entity,
            &GameNpc,
            &RoutineEntry,
            Option<&AnimationController>,
            Option<&mut RoutineExecutor>,
        ),
        (
            Or<(Changed<RoutineEntry>, Added<AnimationController>)>,
            // Routine npcs are world root entities, this also excludes the player visual
            // entity which is a child of the player controller and also carries `GameNpc`.
            Without<ChildOf>,
        ),
    >,
) {
    let mut script = gothic_script.lock();
    for (entity, npc, entry, controller, executor) in &mut npcs {
        if controller.is_none() {
            continue;
        }
        let instance = npc.instance_id();
        // A talking npc resumes its schedule when the dialog closes.
        if dialog_ui.talking_npc() == Some(instance) {
            continue;
        }
        let desired = entry.state_fn;
        let Some(mut executor) = executor else {
            let Some(start) = desired else {
                continue;
            };
            let functions = script.resolve_routine_functions(start);
            script.call_routine_start(instance, functions);
            commands.entity(entity).insert(RoutineExecutor {
                functions,
                source: RoutineSource::Routine,
                way_point: entry.way_point.clone(),
                state_time: 0.0,
                // The engine runs the first loop on the next tick, the loop
                // system picks it up on the next update.
                next_loop_in: 0.0,
            });
            continue;
        };

        if executor.source == RoutineSource::AiState {
            // AI states outlive routine entries. A visual respawn does not
            // re-run the start hook, force one loop so scripts that request
            // their pose from the loop can restore it.
            executor.bypass_change_detection().next_loop_in = 0.0;
            continue;
        }
        if Some(executor.functions.start) == desired && executor.way_point == entry.way_point {
            executor.bypass_change_detection().next_loop_in = 0.0;
            continue;
        }

        // A state start does not clear the queue: routine requests are only
        // produced while it is empty, so there is nothing routine-specific to
        // drop here and explicit actions must survive a routine entry change.
        script.call_routine_end(instance, executor.functions);
        let Some(start) = desired else {
            commands.entity(entity).remove::<RoutineExecutor>();
            continue;
        };
        let functions = script.resolve_routine_functions(start);
        script.call_routine_start(instance, functions);
        executor.functions = functions;
        executor.source = RoutineSource::Routine;
        executor.way_point = entry.way_point.clone();
        executor.state_time = 0.0;
        executor.next_loop_in = 0.0;
    }
}

/// Applies `AI_StartState` requests. The requested state replaces the running
/// executor and outlives routine entry changes until it ends.
#[allow(clippy::type_complexity)]
pub(crate) fn apply_start_state_changes(
    mut commands: Commands,
    gothic_script: Res<ScriptResource>,
    mut changes: MessageReader<StartStateChange>,
    index: Res<InstanceIndex>,
    mut npcs: Query<(
        Entity,
        Option<&AnimationController>,
        Option<&mut RoutineExecutor>,
    )>,
) {
    if changes.is_empty() {
        return;
    }

    let mut script = gothic_script.lock();
    for change in changes.read() {
        for &entity in index.npcs(change.instance_id) {
            let Ok((entity, controller, executor)) = npcs.get_mut(entity) else {
                continue;
            };
            if controller.is_none() {
                continue;
            }
            let functions = script.resolve_routine_functions(change.state);
            if let Some(mut executor) = executor {
                if executor.source == RoutineSource::AiState
                    && executor.functions.start == change.state
                {
                    continue;
                }
                script.call_routine_end(change.instance_id, executor.functions);
                if let Some(other) = change.other {
                    script.set_other(other);
                }
                script.call_routine_start(change.instance_id, functions);
                executor.functions = functions;
                executor.source = RoutineSource::AiState;
                executor.way_point = StringCanon::default();
                executor.state_time = 0.0;
                executor.next_loop_in = 0.0;
            } else {
                if let Some(other) = change.other {
                    script.set_other(other);
                }
                script.call_routine_start(change.instance_id, functions);
                commands.entity(entity).insert(RoutineExecutor {
                    functions,
                    source: RoutineSource::AiState,
                    way_point: StringCanon::default(),
                    state_time: 0.0,
                    next_loop_in: 0.0,
                });
            }
        }
    }
}

/// Publishes the world spots (freepoints) for the freepoint host functions.
pub(crate) fn sync_script_freepoints(
    gothic_script: Option<Res<ScriptResource>>,
    spots: Res<Spots>,
    mut initialized: Local<bool>,
) {
    if *initialized && !spots.is_changed() {
        return;
    }
    let Some(gothic_script) = gothic_script else {
        return;
    };
    if spots.0.is_empty() {
        return;
    }
    let freepoints: Vec<ScriptFreePoint> = spots
        .0
        .iter()
        .map(|(name, tr)| ScriptFreePoint {
            name: name.clone(),
            position: tr.translation,
        })
        .collect();
    debug!("published {} freepoints", freepoints.len());
    gothic_script.lock().set_freepoints(freepoints);
    *initialized = true;
}

/// Frees the freepoint of an npc whose routine state changed or ended.
pub(crate) fn release_npc_freepoints(
    gothic_script: Res<ScriptResource>,
    changed: Query<&GameNpc, Changed<RoutineExecutor>>,
    mut removed: RemovedComponents<RoutineExecutor>,
    npcs: Query<&GameNpc>,
) {
    let mut removed_entities: Vec<Entity> = removed.read().collect();
    if changed.is_empty() && removed_entities.is_empty() {
        return;
    }
    let mut script = gothic_script.lock();
    for npc in &changed {
        script.release_freepoint(npc.instance_id());
    }
    for entity in removed_entities.drain(..) {
        let Ok(npc) = npcs.get(entity) else {
            continue;
        };
        script.release_freepoint(npc.instance_id());
    }
}

/// Calls the loop hook of every running routine state once per interval and
/// ends the state when the loop returns `LOOP_END`.
pub(crate) fn run_routine_loops(
    time: Res<Time>,
    game_time: Res<GameTime>,
    mut commands: Commands,
    gothic_script: Res<ScriptResource>,
    dialog_ui: Res<DialogUiState>,
    mut npcs: Query<
        (
            Entity,
            &GameNpc,
            &mut RoutineExecutor,
            Option<&RoutineEntry>,
            Option<&NpcActionQueue>,
        ),
        With<AnimationController>,
    >,
) {
    if npcs.is_empty() {
        return;
    }

    let delta = time.delta_secs();
    let minutes = game_time.minutes_of_day();
    let talking_npc = dialog_ui.talking_npc();
    let mut script = gothic_script.lock();
    for (entity, npc, mut executor, entry, queue) in &mut npcs {
        // Routine entries stop at their stop time even when no next entry is
        // active yet; the end hook releases a used mob or path.
        if executor.source == RoutineSource::Routine
            && let Some(entry) = entry
            && !entry.is_active_at(minutes)
        {
            debug!(
                "npc({}) routine entry stopped at minute {minutes}",
                npc.instance_id()
            );
            script.call_routine_end(npc.instance_id(), executor.functions);
            commands.entity(entity).remove::<RoutineExecutor>();
            continue;
        }
        // Timer bookkeeping must not mark the executor changed, otherwise
        // `restore_routine_stand` would reset the pose every frame.
        {
            let executor = executor.bypass_change_detection();
            executor.state_time += delta;
            executor.next_loop_in -= delta;
        }
        if executor.next_loop_in > 0.0 {
            continue;
        }
        // A daily routine runs only when the npc has no other action. The loop
        // is paused, not the entry scheduling above; it resumes once the queue
        // drains. Talking pauses it the same way.
        if executor.source == RoutineSource::Routine
            && (talking_npc == Some(npc.instance_id())
                || queue.is_some_and(|queue| !queue.is_empty()))
        {
            continue;
        }
        executor.bypass_change_detection().next_loop_in = ROUTINE_LOOP_INTERVAL_SECS;

        let instance = npc.instance_id();
        let state_time = executor.state_time as i32;
        script.set_routine_state_time(instance, state_time);
        let ended = script.call_routine_loop(instance, executor.functions);
        if let Some(actual) = script.routine_state_time(instance)
            && actual != state_time
        {
            // The script reset the state time via `NPC_SetStateTime`.
            executor.bypass_change_detection().state_time = actual as f32;
        }
        if ended {
            script.call_routine_end(instance, executor.functions);
            if executor.source == RoutineSource::AiState
                && let Some(entry) = entry
                && entry.is_active_at(minutes)
                && let Some(start) = entry.state_fn
            {
                // The AI state returned control, resume the scheduled routine.
                let functions = script.resolve_routine_functions(start);
                script.call_routine_start(instance, functions);
                executor.functions = functions;
                executor.source = RoutineSource::Routine;
                executor.way_point = entry.way_point.clone();
                executor.state_time = 0.0;
                executor.next_loop_in = 0.0;
                continue;
            }
            commands.entity(entity).remove::<RoutineExecutor>();
        }
    }
}

/// Restores the standing idle when a routine state starts or ends. A state that
/// requests no animation leaves the npc standing; requested animations arrive
/// through the script bridge.
#[allow(clippy::type_complexity)]
pub(crate) fn restore_routine_stand(
    mut commands: Commands,
    files: Res<AnimationFiles>,
    mut handles: ResMut<CachedAssetHandles>,
    asset_server: Res<AssetServer>,
    mut removed: RemovedComponents<RoutineExecutor>,
    mut changed: Query<
        (
            Entity,
            &NpcModel,
            Option<&NpcOverlay>,
            &mut AnimationController,
        ),
        (Changed<RoutineExecutor>, With<RoutineExecutor>),
    >,
    mut removed_npcs: Query<
        (&NpcModel, Option<&NpcOverlay>, &mut AnimationController),
        Without<RoutineExecutor>,
    >,
) {
    for (entity, model, overlay, mut controller) in &mut changed {
        // A new routine state drops the previous item pose (`Npc::clearState`
        // does `putState(0,0)`), the standing idle follows below.
        commands.entity(entity).remove::<NpcItemUse>();
        queue_stand_animation(
            &mut controller,
            model,
            overlay,
            &files,
            &mut handles,
            &asset_server,
        );
    }
    for entity in removed.read() {
        let Ok((model, overlay, mut controller)) = removed_npcs.get_mut(entity) else {
            // Despawned npcs drop their item pose with the entity.
            continue;
        };
        commands.entity(entity).remove::<NpcItemUse>();
        queue_stand_animation(
            &mut controller,
            model,
            overlay,
            &files,
            &mut handles,
            &asset_server,
        );
    }
}

/// Standing body state animation fallback, `S_RUN` is the human idle.
pub(crate) const STANDING_ANIMATION: &str = "S_RUN";

/// Entity the npc currently looks at (`AI_LookAtNPC`), maintained by the script
/// bridge. Absent when the npc looks at nothing.
#[derive(Component)]
pub struct NpcLookAt(pub Entity);

/// Rotates npcs to face their [`NpcLookAt`] target. The player visual (a child) is
/// skipped on purpose: rotating it would fight the player controller. When the
/// target entity is gone the npc stops looking. The target is maintained by the
/// script bridge.
pub(crate) fn apply_look_at(
    mut commands: Commands,
    mut npcs: Query<
        (Entity, &NpcLookAt, &GlobalTransform, &mut Transform),
        Without<PlayerCharacter>,
    >,
    transforms: Query<&GlobalTransform>,
) {
    for (entity, look_at, npc_tr, mut tr) in &mut npcs {
        let Ok(target_tr) = transforms.get(look_at.0) else {
            // Target entity was destroyed, stop looking at it
            commands.entity(entity).remove::<NpcLookAt>();
            continue;
        };
        let direction = target_tr.translation() - npc_tr.translation();
        if direction.length_squared() < 1e-4 {
            continue;
        }
        tr.rotation = face_direction_quat(direction);
    }
}

/// Rotates npcs once to face their `AI_TurnToNPC` target, including the player.
pub(crate) fn apply_turn_to_npc(
    mut changes: MessageReader<TurnToNpcChange>,
    index: Res<InstanceIndex>,
    mut npcs: Query<(&GlobalTransform, &mut Transform)>,
    targets: Query<&GlobalTransform>,
) {
    for change in changes.read() {
        let Some(target_entity) = index.npcs(change.target).first().copied() else {
            warn!(
                "AI_TurnToNPC target({}) has no entity, npc({})",
                change.target, change.instance_id
            );
            continue;
        };
        let Ok(target) = targets.get(target_entity) else {
            continue;
        };
        for &entity in index.npcs(change.instance_id) {
            let Ok((npc_global, mut tr)) = npcs.get_mut(entity) else {
                continue;
            };
            let direction = target.translation() - npc_global.translation();
            if direction.length_squared() < 1e-4 {
                continue;
            }
            tr.rotation = face_direction_quat(direction);
        }
    }
}

/// Distance to the next path node under which the npc advances along the path.
const PATH_NODE_REACHED_DISTANCE: f32 = 0.5;

/// Remaining waypoint node indices an npc walks through (`AI_GoToWP`).
#[derive(Component, Default)]
pub(crate) struct NpcPath {
    nodes: Vec<usize>,
    /// World position walked to after the last graph node, used for targets that
    /// are not part of the way net (spots).
    target: Option<Vec3>,
}

impl NpcPath {
    pub(crate) fn new(nodes: Vec<usize>) -> Self {
        Self {
            nodes,
            target: None,
        }
    }

    pub(crate) fn with_target(nodes: Vec<usize>, target: Vec3) -> Self {
        Self {
            nodes,
            target: Some(target),
        }
    }
}

/// Moves npcs along their [`NpcPath`]. Npcs with an animation controller are
/// moved by the root motion of the run animation through the
/// [`AutoAnimSequencer`]; npcs whose visual was never spawned are stepped
/// directly. The npc faces the next node, keeps its height from the path
/// nodes and restores the standing idle on arrival. The body offset matches npc
/// placement, otherwise they would float above the ground while moving.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(crate) fn move_along_path(
    graph: Res<WaypointGraph>,
    files: Res<AnimationFiles>,
    mut handles: ResMut<CachedAssetHandles>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
    mut desired: MessageWriter<SetDesiredAnimation>,
    mut npcs: Query<(
        Entity,
        &mut Transform,
        &mut NpcPath,
        &NpcModel,
        Option<&NpcOverlay>,
        Option<&AutoAnimSequencer>,
        &mut AnimationController,
    )>,
) {
    for (entity, mut transform, mut path, model, overlay, sequencer, mut controller) in &mut npcs {
        let Some(direction) = direction_to_next_node(&mut transform, &mut path, &graph) else {
            // The path is done, restore the standing idle
            commands.entity(entity).remove::<NpcPath>();
            commands.entity(entity).remove::<AutoAnimSequencer>();
            controller.set_root_motion(false);
            if sequencer.is_some() {
                queue_stand_animation(
                    &mut controller,
                    model,
                    overlay,
                    &files,
                    &mut handles,
                    &asset_server,
                );
            }
            continue;
        };
        transform.rotation = face_direction_quat(direction);

        if sequencer.is_none() {
            // Prepare the sequencer first, the desired animation message can
            // only be consumed once the component is actually inserted
            controller.set_root_motion(true);
            commands.entity(entity).insert(AutoAnimSequencer::default());
            continue;
        }

        // `force_change` cuts the current animation immediately and re-queues
        // every frame, which also covers animations still loading
        desired.write(SetDesiredAnimation {
            who: entity,
            animation: DesiredAnimation::Run,
            force_change: true,
            direction: None,
        });
    }
}

/// Removes already reached nodes from the front of the path and returns the
/// direction to the next one, `None` once the path is complete. The npc height
/// is taken from the current path node, root motion only moves it horizontally.
fn direction_to_next_node(
    transform: &mut Transform,
    path: &mut NpcPath,
    graph: &WaypointGraph,
) -> Option<Vec3> {
    while let Some(node) = path.nodes.first().copied() {
        let target = graph.nodes[node].position + Vec3::Y * NPC_BODY_Y_OFFSET;
        transform.translation.y = target.y;
        let to_target = target - transform.translation;
        let distance = to_target.length();
        if distance <= PATH_NODE_REACHED_DISTANCE {
            path.nodes.remove(0);
            continue;
        }
        return Some(to_target / distance);
    }
    let target = path.target?;
    transform.translation.y = target.y;
    let to_target = target - transform.translation;
    let distance = to_target.length();
    if distance <= PATH_NODE_REACHED_DISTANCE {
        path.target = None;
        return None;
    }
    Some(to_target / distance)
}

/// Queues an animation resolved through the npc scheme and its overlay.
pub(crate) fn queue_named_animation(
    controller: &mut AnimationController,
    model: &NpcModel,
    overlay: Option<&NpcOverlay>,
    files: &AnimationFiles,
    handles: &mut CachedAssetHandles,
    asset_server: &AssetServer,
    animation: &str,
) {
    let Some(scheme) = model.animation_scheme.as_deref() else {
        return;
    };
    let overlay = overlay.and_then(|overlay| overlay.0.as_deref());
    let scheme = choose_animation_scheme(scheme, overlay, animation, &files.0);
    let path = get_animation_path(scheme, animation);
    controller.queue_next(handles.get_animation_handle(asset_server, &path), true);
}

/// Switches the npc back to its standing idle after a path.
pub(crate) fn queue_stand_animation(
    controller: &mut AnimationController,
    model: &NpcModel,
    overlay: Option<&NpcOverlay>,
    files: &AnimationFiles,
    handles: &mut CachedAssetHandles,
    asset_server: &AssetServer,
) {
    let animation = model
        .stand_animation
        .as_deref()
        .unwrap_or(STANDING_ANIMATION);
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
