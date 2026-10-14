use std::collections::{HashMap, HashSet};

use bevy::ecs::entity::Entities;
use bevy::prelude::*;

use crate::gameplay::game_time::{GameTime, advance_game_time};
use crate::gameplay::objects::{NpcModel, PlayerCharacter};
use crate::graphics::animation::AnimationController;
use crate::zengin::GameNpc;
use crate::zengin::common::{
    animation_file_name, choose_animation_scheme, face_direction_quat, get_animation_path,
};
use crate::zengin::script::component::{ScriptNpcState, ScriptResource};
use crate::zengin::script::state::{OverlayRequest, WeaponMode};
use crate::zengin::script::symbol_id::{FunctionId, InstanceId};
use crate::zengin::waypoint_graph::WaypointGraph;
use crate::zengin::zengin_resources::{MaterialHandles, ZenGinModelComponent};

/// Waypoint transforms of the loaded world. Spots take precedence over way points,
/// same lookup order as the initial npc placement.
#[derive(Resource, Default)]
pub struct WayPoints {
    pub points: HashMap<String, Transform>,
}

/// Matches the body offset applied when npcs are placed at world load.
const NPC_BODY_Y_OFFSET: f32 = -0.5;

#[derive(Component, Default)]
struct NpcRoutineState {
    /// Waypoint the npc was last assigned to by a routine check,
    /// `None` until the first check.
    assigned_waypoint: Option<String>,
}

#[derive(Component, Default)]
struct NpcRoutineAnimation {
    /// State function of the routine entry whose animation was applied last,
    /// `None` when none was applied yet.
    applied_state_fn: Option<FunctionId>,
}

#[derive(Resource, Default)]
struct NpcRoutineCheckState {
    last_checked_minute: Option<u32>,
}

/// Periodically teleports npcs to the waypoints of their active routine entries.
pub struct NpcRoutinesPlugin;

impl Plugin for NpcRoutinesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NpcRoutineCheckState>();
        app.add_systems(
            Update,
            (
                update_npc_routines,
                resolve_npc_overlays,
                resolve_goto_paths,
                update_npc_routine_animations.after(update_npc_routines),
                apply_look_at.after(update_npc_routines),
                apply_align_to_wp.after(apply_look_at),
                apply_weapon_mode.after(update_npc_routine_animations),
                update_npc_weapon,
                move_along_path.after(update_npc_routines),
            )
                .after(advance_game_time),
        );
    }
}

/// Periodically (on every in-game minute change) checks the active routine
/// entry of each npc. When the routine waypoint changed, the npc walks to the
/// new waypoint (falling back to a teleport when no path is available).
fn update_npc_routines(
    mut check_state: ResMut<NpcRoutineCheckState>,
    game_time: Res<GameTime>,
    gothic_script: Res<ScriptResource>,
    way_points: Res<WayPoints>,
    graph: Res<WaypointGraph>,
    mut commands: Commands,
    mut npcs: Query<
        (
            Entity,
            &GameNpc,
            &mut Transform,
            Option<&mut NpcRoutineState>,
        ),
        // Routine npcs are world root entities, this also excludes the player visual
        // entity which is a child of the player controller and also carries `GameNpc`.
        Without<ChildOf>,
    >,
) {
    let minutes = game_time.minutes_of_day();
    if check_state.last_checked_minute == Some(minutes) {
        return;
    }
    let first_check = check_state.last_checked_minute.is_none();
    check_state.last_checked_minute = Some(minutes);

    let gothic_script = gothic_script.lock();
    let loaded_state = gothic_script.get_loaded_state();

    for (entity, npc, mut tr, routine_state) in &mut npcs {
        let Some(instance) = loaded_state.instance_data.get(&npc.npc_instance_id) else {
            continue;
        };
        let Some(entry) = instance.get_routine_entry(minutes) else {
            continue;
        };

        match routine_state {
            Some(mut state) => {
                if state.assigned_waypoint.as_deref() == Some(entry.way_point.as_str()) {
                    continue;
                }
                if first_check {
                    // Npcs were already placed at the start hour by world load,
                    // only record the current assignment.
                    state.assigned_waypoint = Some(entry.way_point.clone());
                    continue;
                }
                state.assigned_waypoint = Some(entry.way_point.clone());
                let Some(waypoint_tr) = way_points.points.get(&entry.way_point) else {
                    crate::warn_once!(
                        "routine waypoint not found, npc({}), waypoint({})",
                        npc.npc_instance_id,
                        entry.way_point
                    );
                    continue;
                };

                // Walk to the waypoint over the graph; teleport when no path exists
                if let (Some(target), Some(start)) = (
                    graph.node_by_name(&entry.way_point),
                    graph.nearest_node(tr.translation),
                ) && let Some(path) = graph.shortest_path(start, target)
                {
                    debug!(
                        "walk npc({}) to waypoint({})",
                        npc.npc_instance_id, entry.way_point
                    );
                    commands.entity(entity).insert(NpcPath {
                        nodes: path,
                        speed: NPC_WALK_SPEED,
                    });
                    continue;
                }

                debug!(
                    "teleport npc({}) to waypoint({})",
                    npc.npc_instance_id, entry.way_point
                );
                // Keep any model scale Mdl_SetModelScale applied
                let scale = tr.scale;
                *tr = Transform::from_translation(
                    waypoint_tr.translation + Vec3::Y * NPC_BODY_Y_OFFSET,
                )
                .with_rotation(waypoint_tr.rotation)
                .with_scale(scale);
            }
            None => {
                // Late inserted npcs are already placed at the current routine waypoint
                // by the world load, only record the assignment.
                commands.entity(entity).insert(NpcRoutineState {
                    assigned_waypoint: Some(entry.way_point.clone()),
                });
            }
        }
    }
}

/// Applies the animation of the active routine entry state to npcs. The
/// routine state function (`ZS_*`) is executed in the script vm, animation
/// names it requests via `AI_PlayAni(BS)` are collected and the resulting
/// animation is switched to on the npc visual. The state function is only
/// re-executed when the active routine entry state changes, so the per frame
/// cost for already applied npcs is just the active entry lookup.
#[allow(clippy::type_complexity)]
fn update_npc_routine_animations(
    game_time: Res<GameTime>,
    gothic_script: Res<ScriptResource>,
    files: Res<AnimationFiles>,
    mut handles: ResMut<MaterialHandles>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
    mut npcs: Query<
        (
            Entity,
            &GameNpc,
            &NpcModel,
            Option<&NpcOverlay>,
            Option<&mut NpcRoutineAnimation>,
            Option<&mut AnimationController>,
        ),
        // Routine npcs are world root entities, this also excludes the player visual
        // entity which is a child of the player controller and also carries `GameNpc`.
        Without<ChildOf>,
    >,
) {
    let minutes = game_time.minutes_of_day();
    let mut gothic_script = gothic_script.lock();

    for (entity, npc, npc_model, overlay, routine_animation, controller) in &mut npcs {
        let Some(mut routine_animation) = routine_animation else {
            commands
                .entity(entity)
                .insert(NpcRoutineAnimation::default());
            continue;
        };
        let Some(mut controller) = controller else {
            // Npc visual is not spawned yet, retry when it is
            continue;
        };
        let Some(scheme) = npc_model.animation_scheme.as_deref() else {
            continue;
        };
        let stand_animation = npc_model
            .stand_animation
            .as_deref()
            .unwrap_or(STANDING_ANIMATION);

        let state_fn = {
            let loaded_state = gothic_script.get_loaded_state();
            let Some(entry) = loaded_state
                .instance_data
                .get(&npc.npc_instance_id)
                .and_then(|instance| instance.get_routine_entry(minutes))
            else {
                continue;
            };
            let Some(state_fn) = entry.state_fn else {
                continue;
            };
            if routine_animation.applied_state_fn == Some(state_fn) {
                continue;
            }
            state_fn
        };

        routine_animation.applied_state_fn = Some(state_fn);
        let played_anis = gothic_script.call_routine_state_function(npc.npc_instance_id, state_fn);
        let ani = choose_routine_animation(&played_anis, stand_animation);

        let overlay = overlay.and_then(|overlay| overlay.0.as_deref());
        let scheme = choose_animation_scheme(scheme, overlay, &ani, &files.0);
        let path = get_animation_path(scheme, &ani);
        // info!("npc({}) routine animation({})", npc.npc_instance_id, ani);
        // Applied via `next` so the animation system syncs its playback state on
        // the switch. When the animation asset does not exist the switch is
        // silently dropped and the npc keeps its current animation.
        controller.next = Some(handles.get_animation_handle(&asset_server, &path));
        controller.interrupt_current = true;
    }
}

/// Standing body state animation fallback, `S_RUN` is the human idle.
const STANDING_ANIMATION: &str = "S_RUN";

/// Picks the animation to loop from the names the routine state script
/// requested. The engine keeps the body state animation looping after a
/// transition, so a requested transition into a pose (`T_X_2_Y`) is replaced
/// by the pose idle itself (`S_Y`). States that request no animation
/// (`played_anis` empty) leave the npc in the default standing body state, so
/// they fall back to the scheme standing idle (`stand_animation`) too.
fn choose_routine_animation(played_anis: &[String], stand_animation: &str) -> String {
    let Some(ani) = played_anis.first() else {
        return stand_animation.to_string();
    };
    match ani.rsplit_once("_2_") {
        Some((_, "STAND")) => stand_animation.to_string(),
        Some((_, pose)) => format!("S_{pose}"),
        None => ani.clone(),
    }
}

/// Rotates npcs to face their `AI_LookAtNPC` target. The player visual (a child)
/// is skipped on purpose: rotating it would fight the player controller. Locks
/// only the shared npc state, never the script vm.
fn apply_look_at(
    state: Res<ScriptNpcState>,
    mut npcs: Query<(&GameNpc, &mut Transform), Without<PlayerCharacter>>,
) {
    let Ok(live) = state.0.lock() else {
        return;
    };
    if live.look_at.is_empty() {
        return;
    }

    for (npc, mut tr) in &mut npcs {
        let Some(target) = live.look_at.get(&npc.npc_instance_id) else {
            continue;
        };
        let (Some(from), Some(to)) = (
            live.positions.get(&npc.npc_instance_id),
            live.positions.get(target),
        ) else {
            continue;
        };
        let direction = *to - *from;
        if direction.length_squared() < 1e-4 {
            continue;
        }
        tr.rotation = face_direction_quat(direction);
    }
}

#[derive(Component, Default)]
struct NpcWeaponMode {
    /// Weapon mode whose idle animation was applied last
    applied: Option<WeaponMode>,
}

/// Switches the idle animation of npcs whose weapon mode changed (drawn melee
/// uses the armed idle `S_1H`, otherwise the scheme standing idle). Only plays an
/// animation when the mode actually changed. Locks only the shared npc state.
#[allow(clippy::type_complexity)]
fn apply_weapon_mode(
    state: Res<ScriptNpcState>,
    files: Res<AnimationFiles>,
    mut handles: ResMut<MaterialHandles>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
    mut npcs: Query<(
        Entity,
        &GameNpc,
        &NpcModel,
        Option<&NpcOverlay>,
        Option<&mut NpcWeaponMode>,
        Option<&mut AnimationController>,
    )>,
) {
    let Ok(live) = state.0.lock() else {
        return;
    };
    if live.weapon_mode.is_empty() {
        return;
    }

    for (entity, npc, model, overlay, mode_comp, controller) in &mut npcs {
        let Some(weapon_mode) = live.weapon_mode.get(&npc.npc_instance_id).copied() else {
            continue;
        };
        let Some(mut applied) = mode_comp else {
            commands.entity(entity).insert(NpcWeaponMode::default());
            continue;
        };
        let Some(mut controller) = controller else {
            continue;
        };
        if applied.applied == Some(weapon_mode) {
            continue;
        }
        applied.applied = Some(weapon_mode);

        let Some(scheme) = model.animation_scheme.as_deref() else {
            continue;
        };
        let animation = match weapon_mode {
            WeaponMode::Melee => "S_1H".to_string(),
            WeaponMode::Fists => {
                // No resolved standing idle, keep the current animation
                let Some(stand) = model.stand_animation.clone() else {
                    continue;
                };
                stand
            }
        };
        let overlay = overlay.and_then(|overlay| overlay.0.as_deref());
        let scheme = choose_animation_scheme(scheme, overlay, &animation, &files.0);
        let path = get_animation_path(scheme, &animation);
        controller.next = Some(handles.get_animation_handle(&asset_server, &path));
        controller.interrupt_current = true;
    }
}

/// Faces npcs at their assigned routine waypoint when the script requested it
/// via `AI_AlignToWP`; the waypoint rotation is the facing direction. Consumes
/// one request per npc. Locks only the shared npc state.
fn apply_align_to_wp(
    state: Res<ScriptNpcState>,
    way_points: Res<WayPoints>,
    mut npcs: Query<(&GameNpc, Option<&NpcRoutineState>, &mut Transform)>,
) {
    let Ok(mut live) = state.0.lock() else {
        return;
    };
    if live.align_to_wp.is_empty() {
        return;
    }

    for (npc, routine, mut tr) in &mut npcs {
        if !live.align_to_wp.remove(&npc.npc_instance_id) {
            continue;
        }
        let Some(name) = routine.and_then(|routine| routine.assigned_waypoint.as_deref()) else {
            continue;
        };
        let Some(way_point) = way_points.points.get(name) else {
            continue;
        };
        tr.rotation = way_point.rotation;
    }
}

/// Engine weapon slot nodes of an npc model, `hip` is the sheathed position
/// (`ZS_SWORD`), `hand` the drawn one (`ZS_RIGHTHAND`).
#[derive(Component, Default)]
pub struct NpcWeaponSlots {
    pub hip: Option<Entity>,
    pub hand: Option<Entity>,
}

/// Weapon mesh currently attached to an npc, managed by [`update_npc_weapon`].
#[derive(Component, Default)]
struct NpcWeapon {
    entity: Option<Entity>,
    item: Option<InstanceId>,
    slot: Option<Entity>,
}

/// Item symbol id to weapon mesh asset path, built at world load.
#[derive(Resource, Default)]
pub struct WeaponMeshPaths(pub HashMap<i32, String>);

/// Keeps a weapon mesh on each npc that has one equipped. The default position
/// is the hip, switching to melee mode moves it to the hand. Reacts to a runtime
/// `EquipItem`. Locks only the shared npc state.
#[allow(clippy::type_complexity)]
fn update_npc_weapon(
    state: Res<ScriptNpcState>,
    mesh_paths: Res<WeaponMeshPaths>,
    mut handles: ResMut<MaterialHandles>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
    entities: &Entities,
    mut npcs: Query<(Entity, &GameNpc, &NpcWeaponSlots, Option<&mut NpcWeapon>)>,
) {
    let Ok(live) = state.0.lock() else {
        return;
    };

    for (entity, npc, slots, weapon) in &mut npcs {
        let item = live.equipped_weapon.get(&npc.npc_instance_id).copied();
        let drawn = matches!(
            live.weapon_mode.get(&npc.npc_instance_id),
            Some(WeaponMode::Melee)
        );
        // The joints are despawned with the visual when the npc is far
        let target_slot =
            if drawn { slots.hand } else { slots.hip }.filter(|slot| entities.contains(*slot));

        let Some(mut weapon) = weapon else {
            commands.entity(entity).insert(NpcWeapon::default());
            continue;
        };

        let Some(path) = item
            .and_then(|item| mesh_paths.0.get(&item.value()))
            .cloned()
        else {
            // Nothing equipped, or its mesh is unknown
            if let Some(child) = weapon.entity.take()
                && entities.contains(child)
            {
                commands.entity(child).despawn();
            }
            weapon.item = None;
            weapon.slot = None;
            continue;
        };

        let child_alive = weapon.entity.is_some_and(|child| entities.contains(child));
        if !child_alive {
            // Weapon child was despawned (npc went far) or never spawned
            weapon.entity = None;
            weapon.slot = None;
            let Some(target_slot) = target_slot else {
                continue;
            };
            let handle = handles.get_model_handle(&asset_server, &path, None);
            let child = commands
                .spawn((
                    Visibility::default(),
                    ZenGinModelComponent {
                        model_handle: handle,
                        ..default()
                    },
                    Transform::IDENTITY,
                    ChildOf(target_slot),
                ))
                .id();
            weapon.entity = Some(child);
            weapon.item = item;
            weapon.slot = Some(target_slot);
            continue;
        }

        weapon.item = item;

        // Move between sheathed and drawn
        if weapon.slot != target_slot
            && let Some(child) = weapon.entity
            && let Some(target_slot) = target_slot
        {
            commands.entity(child).insert(ChildOf(target_slot));
            weapon.slot = Some(target_slot);
        }
    }
}

/// Overlay animation scheme currently applied to an npc (`Mdl_ApplyOverlayMds`).
#[derive(Component, Default)]
pub struct NpcOverlay(pub Option<String>);

/// File names in the compiled anim archive, used to resolve overlay animations.
#[derive(Resource, Default)]
pub struct AnimationFiles(pub HashSet<String>);

/// Strips a model file extension and uppercases: `Humans_Relaxed.mds` ->
/// `HUMANS_RELAXED`.
fn overlay_scheme(name: &str) -> String {
    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    stem.to_uppercase()
}

/// Resolves pending `Mdl_ApplyOverlayMds` requests to a scheme string. The symbol
/// name is read through the script vm, which is only safe outside a running
/// script, hence this runs in `Update`. Locks the shared state and the script,
/// but never both at once.
fn resolve_npc_overlays(
    gothic_script: Res<ScriptResource>,
    state: Res<ScriptNpcState>,
    mut commands: Commands,
    npcs: Query<(Entity, &GameNpc)>,
) {
    let requests: Vec<(InstanceId, OverlayRequest)> = {
        let Ok(mut live) = state.0.lock() else {
            return;
        };
        live.overlay_request.drain().collect()
    };
    if requests.is_empty() {
        return;
    }

    let mut resolved: Vec<(InstanceId, Option<String>)> = Vec::with_capacity(requests.len());
    {
        let mut gothic_script = gothic_script.lock();
        for (npc, request) in requests {
            let scheme = match request {
                OverlayRequest::Apply(id) => gothic_script
                    .get_name_by_id(id)
                    .map(|name| overlay_scheme(&name)),
                OverlayRequest::Remove => None,
            };
            resolved.push((npc, scheme));
        }
    }

    let schemes: HashMap<InstanceId, Option<String>> = resolved.into_iter().collect();
    for (entity, npc) in &npcs {
        if let Some(scheme) = schemes.get(&npc.npc_instance_id) {
            commands.entity(entity).insert(NpcOverlay(scheme.clone()));
        }
    }
}

/// Walking speed of npcs following an `AI_GoToWP` path, in Bevy meters/second.
const NPC_WALK_SPEED: f32 = 3.0;

/// Remaining waypoint node indices an npc walks through (`AI_GoToWP`).
#[derive(Component, Default)]
struct NpcPath {
    nodes: Vec<usize>,
    speed: f32,
}

/// Resolves `AI_GoToWP` requests into a path over the waypoint graph. Reads the
/// waypoint name through the script vm, which is only safe outside a running
/// script, hence this runs in `Update`. Locks the shared state and the script,
/// but never both at once.
fn resolve_goto_paths(
    gothic_script: Res<ScriptResource>,
    state: Res<ScriptNpcState>,
    graph: Res<WaypointGraph>,
    mut commands: Commands,
    npcs: Query<(Entity, &GameNpc, &Transform)>,
) {
    let requests: Vec<(InstanceId, i32)> = {
        let Ok(mut live) = state.0.lock() else {
            return;
        };
        live.goto_request.drain().collect()
    };
    if requests.is_empty() {
        return;
    }

    let names: HashMap<InstanceId, Option<String>> = {
        let mut gothic_script = gothic_script.lock();
        requests
            .into_iter()
            .map(|(npc, id)| (npc, gothic_script.get_name_by_id(id)))
            .collect()
    };

    for (entity, npc, transform) in &npcs {
        let Some(Some(name)) = names.get(&npc.npc_instance_id) else {
            continue;
        };
        let Some(target) = graph.node_by_name(name) else {
            continue;
        };
        let Some(start) = graph.nearest_node(transform.translation) else {
            continue;
        };
        let Some(path) = graph.shortest_path(start, target) else {
            continue;
        };
        commands.entity(entity).insert(NpcPath {
            nodes: path,
            speed: NPC_WALK_SPEED,
        });
    }
}

/// Walk-loop animations tried while path-following, in preference order; the
/// first one the scheme (or its overlay) has is played.
const WALK_ANIMATIONS: [&str; 4] = ["S_RUNL", "S_FISTRUNL", "S_WALKL", "S_FISTWALKL"];

/// Tracks whether the walk animation is currently applied to an npc.
#[derive(Component, Default)]
struct NpcWalk {
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
fn move_along_path(
    time: Res<Time>,
    graph: Res<WaypointGraph>,
    files: Res<AnimationFiles>,
    mut handles: ResMut<MaterialHandles>,
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
                controller.next = Some(handles.get_animation_handle(&asset_server, &path));
                controller.interrupt_current = true;
            }
        } else if arrived && walk.walking {
            walk.walking = false;
            let animation = model
                .stand_animation
                .as_deref()
                .unwrap_or(STANDING_ANIMATION);
            let scheme = choose_animation_scheme(scheme, overlay, animation, &files.0);
            let path = get_animation_path(scheme, animation);
            controller.next = Some(handles.get_animation_handle(&asset_server, &path));
            controller.interrupt_current = true;
        }
    }
}
