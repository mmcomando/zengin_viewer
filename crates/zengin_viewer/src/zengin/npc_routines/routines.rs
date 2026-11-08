//! Waypoint routines: active routine entries, their waypoint targets,
//! routine state animations and `AI_GoToWP` path following.

use std::collections::{HashMap, HashSet};
use std::sync::PoisonError;

use bevy::prelude::*;

use crate::gameplay::game_time::GameTime;
use crate::gameplay::objects::{NpcModel, PlayerCharacter};
use crate::graphics::AnimationController;
use crate::zengin::GameNpc;
use crate::zengin::common::{
    animation_file_name, choose_animation_scheme, face_direction_quat, get_animation_path,
};
use crate::zengin::npc_actions::{NpcAction, NpcActionQueue};
use crate::zengin::npc_routines::overlay::{AnimationFiles, NpcOverlay};
use crate::zengin::resources::CachedAssetHandles;
use crate::zengin::script::component::ScriptResource;
use crate::zengin::script::state::RoutineEntry;
use crate::zengin::script::string_value::StringCanon;
use crate::zengin::script::symbol_id::FunctionId;
use crate::zengin::waypoint_graph::WaypointGraph;

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

#[derive(Component, Default)]
pub(crate) struct NpcRoutineAnimation {
    /// State function of the routine entry whose animation was applied last,
    /// `None` when none was applied yet.
    applied_state_fn: Option<FunctionId>,
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
    for entity in removed.read() {
        // Despawned entities are fine here (e.g. a killed npc), `try_remove`
        // treats the command as a no-op for them.
        commands.entity(entity).try_remove::<NpcRoutineState>();
    }

    for (entity, npc, entry, routine_state, queue) in &mut npcs {
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

        let Some(mut queue) = queue else {
            crate::warn_once!(
                "routine npc({}) has no action queue, waypoint({})",
                npc.instance_id(),
                entry.way_point
            );
            continue;
        };
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
}

/// Applies the animation of the active routine entry state to npcs. The
/// routine state function (`ZS_*`) is executed in the script vm, animation
/// names it requests via `AI_PlayAni(BS)` are collected and the resulting
/// animation is switched to on the npc visual. The state function is only
/// re-executed when the active routine entry state changes or the visual
/// controller appears, so unchanged npcs are not visited at all.
#[allow(clippy::type_complexity)]
pub(crate) fn update_npc_routine_animations(
    gothic_script: Res<ScriptResource>,
    files: Res<AnimationFiles>,
    mut handles: ResMut<CachedAssetHandles>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
    mut npcs: Query<
        (
            Entity,
            &GameNpc,
            &RoutineEntry,
            &NpcModel,
            Option<&NpcOverlay>,
            Option<&mut NpcRoutineAnimation>,
            Option<&mut AnimationController>,
        ),
        (
            Or<(Changed<RoutineEntry>, Added<AnimationController>)>,
            // Routine npcs are world root entities, this also excludes the player visual
            // entity which is a child of the player controller and also carries `GameNpc`.
            Without<ChildOf>,
        ),
    >,
) {
    for (entity, npc, entry, npc_model, overlay, routine_animation, controller) in &mut npcs {
        let Some(mut controller) = controller else {
            // Npc visual is not spawned yet, retry when its controller appears
            continue;
        };
        let Some(scheme) = npc_model.animation_scheme.as_deref() else {
            continue;
        };
        let stand_animation = npc_model
            .stand_animation
            .as_deref()
            .unwrap_or(STANDING_ANIMATION);

        let Some(state_fn) = entry.state_fn else {
            continue;
        };
        let applied = routine_animation
            .as_ref()
            .and_then(|routine_animation| routine_animation.applied_state_fn);
        if applied == Some(state_fn) {
            continue;
        }

        // Only lock the script when a state actually has to run
        let played_anis = gothic_script
            .lock()
            .call_routine_state_function(npc.instance_id(), state_fn);
        let ani = choose_routine_animation(&played_anis, stand_animation);

        let overlay = overlay.and_then(|overlay| overlay.0.as_deref());
        let scheme = choose_animation_scheme(scheme, overlay, &ani, &files.0);
        let path = get_animation_path(scheme, &ani);
        // info!("npc({}) routine animation({})", npc.instance_id(), ani);
        // Applied via `next` so the animation system syncs its playback state on
        // the switch. When the animation asset does not exist the switch is
        // silently dropped and the npc keeps its current animation.
        controller.queue_next(handles.get_animation_handle(&asset_server, &path), true);

        if let Some(mut routine_animation) = routine_animation {
            routine_animation.applied_state_fn = Some(state_fn);
        } else {
            commands.entity(entity).insert(NpcRoutineAnimation {
                applied_state_fn: Some(state_fn),
            });
        }
    }
}

/// Standing body state animation fallback, `S_RUN` is the human idle.
pub(crate) const STANDING_ANIMATION: &str = "S_RUN";

/// Picks the animation to loop from the names the routine state script
/// requested. The engine keeps the body state animation looping after a
/// transition, so a requested transition into a pose (`T_X_2_Y`) is replaced
/// by the pose idle itself (`S_Y`). States that request no animation
/// (`played_anis` empty) leave the npc in the default standing body state, so
/// they fall back to the scheme standing idle (`stand_animation`) too.
fn choose_routine_animation(played_anis: &[StringCanon], stand_animation: &str) -> String {
    let Some(ani) = played_anis.first() else {
        return stand_animation.to_string();
    };
    // Requested ani names are canonically lowercased, poses compare lowercase
    match ani.as_str().rsplit_once("_2_") {
        Some((_, "stand")) => stand_animation.to_string(),
        Some((_, pose)) => format!("S_{}", pose.to_uppercase()),
        None => ani.as_str().to_string(),
    }
}

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

/// Walking speed of npcs following an `AI_GoToWP` path, in Bevy meters/second.
pub(crate) const NPC_WALK_SPEED: f32 = 3.0;

/// Remaining waypoint node indices an npc walks through (`AI_GoToWP`).
#[derive(Component, Default)]
pub(crate) struct NpcPath {
    nodes: Vec<usize>,
    speed: f32,
}

impl NpcPath {
    pub(crate) fn new(nodes: Vec<usize>, speed: f32) -> Self {
        Self { nodes, speed }
    }
}

/// Walk-loop animations tried while path-following, in preference order; the
/// first one the scheme (or its overlay) has is played.
const WALK_ANIMATIONS: [&str; 4] = ["S_RUNL", "S_FISTRUNL", "S_WALKL", "S_FISTWALKL"];

/// Tracks whether the walk animation is currently applied to an npc.
#[derive(Component, Default)]
pub(crate) struct NpcWalk {
    walking: bool,
}

/// First candidate animation the scheme or its overlay actually has.
fn existing_animation<'a>(
    scheme: &'a str,
    overlay: Option<&'a str>,
    candidates: &[&'a str],
    files: &HashSet<String>,
) -> Option<(&'a str, &'a str)> {
    for animation in candidates {
        if let Some(overlay) = overlay
            && files.contains(&animation_file_name(overlay, animation))
        {
            return Some((overlay, animation));
        }
        if files.contains(&animation_file_name(scheme, animation)) {
            return Some((scheme, animation));
        }
    }
    None
}

/// Moves npcs along their [`NpcPath`] at a constant speed, facing the move
/// direction, and plays a walk-loop animation while moving (restoring the
/// standing idle on arrival). The body offset matches npc placement, otherwise
/// they would float above the ground while moving.
#[allow(clippy::type_complexity)]
pub(crate) fn move_along_path(
    time: Res<Time>,
    graph: Res<WaypointGraph>,
    files: Res<AnimationFiles>,
    mut handles: ResMut<CachedAssetHandles>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
    mut npcs: Query<(
        Entity,
        &mut Transform,
        &mut NpcPath,
        &NpcModel,
        Option<&NpcOverlay>,
        Option<&mut AnimationController>,
        Option<&mut NpcWalk>,
    )>,
) {
    let delta = time.delta_secs();
    for (entity, mut transform, mut path, model, overlay, controller, walk) in &mut npcs {
        let step = path.speed * delta;
        while let Some(node) = path.nodes.first().copied() {
            let target = graph.nodes[node].position + Vec3::Y * NPC_BODY_Y_OFFSET;
            let to_target = target - transform.translation;
            let distance = to_target.length();
            if distance <= step {
                transform.translation = target;
                path.nodes.remove(0);
                continue;
            }
            let direction = to_target / distance;
            transform.translation += direction * step;
            transform.rotation = face_direction_quat(direction);
            break;
        }
        let arrived = path.nodes.is_empty();
        if arrived {
            commands.entity(entity).remove::<NpcPath>();
        }

        // Walk animation while moving, standing idle once arrived
        let (Some(mut controller), Some(mut walk)) = (controller, walk) else {
            commands.entity(entity).insert(NpcWalk::default());
            continue;
        };
        let Some(scheme) = model.animation_scheme.as_deref() else {
            continue;
        };
        let overlay = overlay.and_then(|overlay| overlay.0.as_deref());

        if !arrived && !walk.walking {
            walk.walking = true;
            if let Some((scheme, animation)) =
                existing_animation(scheme, overlay, &WALK_ANIMATIONS, &files.0)
            {
                let path = get_animation_path(scheme, animation);
                controller.queue_next(handles.get_animation_handle(&asset_server, &path), true);
            }
        } else if arrived && walk.walking {
            walk.walking = false;
            let animation = model
                .stand_animation
                .as_deref()
                .unwrap_or(STANDING_ANIMATION);
            let scheme = choose_animation_scheme(scheme, overlay, animation, &files.0);
            let path = get_animation_path(scheme, animation);
            controller.queue_next(handles.get_animation_handle(&asset_server, &path), true);
        }
    }
}
