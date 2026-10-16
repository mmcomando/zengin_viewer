use std::collections::{HashMap, HashSet};

use bevy::ecs::entity::Entities;
use bevy::prelude::*;

use crate::gameplay::game_time::{GameTime, advance_game_time};
use crate::gameplay::objects::{NpcModel, PlayerCharacter};
use crate::graphics::animation::AnimationController;
use crate::zengin::GameNpc;
use crate::zengin::ZenginVfs;
use crate::zengin::common::{
    animation_file_name, choose_animation_scheme, face_direction_quat, get_animation_path,
};
use crate::zengin::instance_index::InstanceIndex;
use crate::zengin::script::component::ScriptResource;
use crate::zengin::script::state::{AlignToWpChange, GotoChange, RoutineEntry, WeaponMode};
use crate::zengin::script::symbol_id::{FunctionId, InstanceId};
use crate::zengin::script_bridge::ScriptChangesApplied;
use crate::zengin::waypoint_graph::WaypointGraph;
use crate::zengin::world::item_model_path;
use crate::zengin::zengin_resources::{MaterialHandles, ZenGinModelComponent};

/// Waypoint transforms of the loaded world. Spots take precedence over way points,
/// same lookup order as the initial npc placement.
#[derive(Resource, Default)]
pub struct WayPoints {
    pub points: HashMap<String, Transform>,
}

/// Matches the body offset applied when npcs are placed at world load.
const NPC_BODY_Y_OFFSET: f32 = -0.5;

#[derive(Component)]
struct NpcRoutineState {
    /// Waypoint the npc was last assigned to by a routine check
    assigned_waypoint: String,
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
                (update_npc_routine_entries, update_npc_routines).chain(),
                resolve_goto_paths,
                update_npc_routine_animations.after(update_npc_routines),
                apply_look_at.after(update_npc_routines),
                apply_align_to_wp.after(apply_look_at),
                apply_weapon_mode.after(update_npc_routine_animations),
                update_npc_weapon,
                move_along_path.after(update_npc_routines),
            )
                .after(advance_game_time)
                .after(ScriptChangesApplied),
        );
    }
}

/// Refreshes the active routine entry of already activated npcs on every in-game
/// minute change. Only npcs that already carry a [`RoutineEntry`] component are
/// touched; adding and removing the component is driven by the script bridge.
fn update_npc_routine_entries(
    mut check_state: ResMut<NpcRoutineCheckState>,
    game_time: Res<GameTime>,
    gothic_script: Res<ScriptResource>,
    mut npcs: Query<(&GameNpc, &mut RoutineEntry), Without<ChildOf>>,
) {
    let minutes = game_time.minutes_of_day();
    if check_state.last_checked_minute == Some(minutes) {
        return;
    }
    check_state.last_checked_minute = Some(minutes);

    let gothic_script = gothic_script.lock();
    let instances = gothic_script.npc_instances();
    for (npc, mut entry) in &mut npcs {
        let Some(active) = instances
            .get(&npc.npc_instance_id)
            .and_then(|instance| instance.get_routine_entry(minutes))
        else {
            continue;
        };
        if *entry == *active {
            continue;
        }
        *entry = active.clone();
    }
}

/// Reacts to the [`RoutineEntry`] component of an npc being added, changed or
/// removed. When the active routine entry changed, the npc walks to the new
/// waypoint (falling back to a teleport when no path is available).
fn update_npc_routines(
    mut commands: Commands,
    way_points: Res<WayPoints>,
    graph: Res<WaypointGraph>,
    mut npcs: Query<
        (
            Entity,
            &GameNpc,
            &RoutineEntry,
            &mut Transform,
            Option<&mut NpcRoutineState>,
        ),
        (Changed<RoutineEntry>, Without<ChildOf>),
    >,
    mut removed: RemovedComponents<RoutineEntry>,
) {
    for entity in removed.read() {
        commands.entity(entity).remove::<NpcRoutineState>();
    }

    for (entity, npc, entry, mut tr, routine_state) in &mut npcs {
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
        *tr = Transform::from_translation(waypoint_tr.translation + Vec3::Y * NPC_BODY_Y_OFFSET)
            .with_rotation(waypoint_tr.rotation)
            .with_scale(scale);
    }
}

/// Applies the animation of the active routine entry state to npcs. The
/// routine state function (`ZS_*`) is executed in the script vm, animation
/// names it requests via `AI_PlayAni(BS)` are collected and the resulting
/// animation is switched to on the npc visual. The state function is only
/// re-executed when the active routine entry state changes, so the per frame
/// cost for already applied npcs is just the component lookup.
#[allow(clippy::type_complexity)]
fn update_npc_routine_animations(
    gothic_script: Res<ScriptResource>,
    files: Res<AnimationFiles>,
    mut handles: ResMut<MaterialHandles>,
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
        // Routine npcs are world root entities, this also excludes the player visual
        // entity which is a child of the player controller and also carries `GameNpc`.
        Without<ChildOf>,
    >,
) {
    for (entity, npc, entry, npc_model, overlay, routine_animation, controller) in &mut npcs {
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

        let Some(state_fn) = entry.state_fn else {
            continue;
        };
        if routine_animation.applied_state_fn == Some(state_fn) {
            continue;
        }

        routine_animation.applied_state_fn = Some(state_fn);
        // Only lock the script when a state actually has to run
        let played_anis = gothic_script
            .lock()
            .call_routine_state_function(npc.npc_instance_id, state_fn);
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

/// Entity the npc currently looks at (`AI_LookAtNPC`), maintained by the script
/// bridge. Absent when the npc looks at nothing.
#[derive(Component)]
pub struct NpcLookAt(pub Entity);

/// Rotates npcs to face their [`NpcLookAt`] target. The player visual (a child) is
/// skipped on purpose: rotating it would fight the player controller. When the
/// target entity is gone the npc stops looking. The target is maintained by the
/// script bridge.
fn apply_look_at(
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

/// Weapon mode of the npc (`Npc_SetToFightMode`/`SetToFistMode`/weapon AI),
/// maintained by the script bridge.
#[derive(Component)]
pub struct NpcWeaponMode(pub WeaponMode);

/// Switches the idle animation of an npc when its weapon mode changes (drawn
/// melee uses the armed idle `S_1H`, otherwise the scheme standing idle). Also
/// applies once when the visual (animation controller) appears.
#[allow(clippy::type_complexity)]
fn apply_weapon_mode(
    files: Res<AnimationFiles>,
    mut handles: ResMut<MaterialHandles>,
    asset_server: Res<AssetServer>,
    mut npcs: Query<
        (
            &NpcModel,
            Option<&NpcOverlay>,
            &NpcWeaponMode,
            &mut AnimationController,
        ),
        Or<(Changed<NpcWeaponMode>, Added<AnimationController>)>,
    >,
) {
    for (model, overlay, weapon_mode, mut controller) in &mut npcs {
        let Some(scheme) = model.animation_scheme.as_deref() else {
            continue;
        };
        let animation = match weapon_mode.0 {
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
/// via `AI_AlignToWP`; the waypoint rotation is the facing direction.
fn apply_align_to_wp(
    mut changes: MessageReader<AlignToWpChange>,
    index: Res<InstanceIndex>,
    way_points: Res<WayPoints>,
    mut npcs: Query<(Option<&NpcRoutineState>, &mut Transform)>,
) {
    for change in changes.read() {
        for &entity in index.npcs(change.instance_id) {
            let Ok((routine, mut tr)) = npcs.get_mut(entity) else {
                continue;
            };
            let Some(name) = routine.map(|routine| routine.assigned_waypoint.as_str()) else {
                continue;
            };
            let Some(way_point) = way_points.points.get(name) else {
                continue;
            };
            tr.rotation = way_point.rotation;
        }
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

/// Item equipped by the npc (`EquipItem`), maintained by the script bridge.
/// Absent when the npc has no weapon equipped.
#[derive(Component)]
pub struct NpcEquippedWeapon(pub InstanceId);

/// Keeps a weapon mesh on each npc that has one equipped. The default position
/// is the hip, switching to melee mode moves it to the hand. Reacts to the
/// equipped item changing, the weapon mode changing, and the visual (weapon
/// slots) appearing.
#[allow(clippy::type_complexity)]
fn update_npc_weapon(
    gothic_scripts: Res<ScriptResource>,
    vfs: Res<ZenginVfs>,
    mut handles: ResMut<MaterialHandles>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
    entities: &Entities,
    mut npcs: Query<
        (
            Entity,
            &NpcWeaponSlots,
            Option<&NpcEquippedWeapon>,
            Option<&NpcWeaponMode>,
            Option<&mut NpcWeapon>,
        ),
        Or<(
            Changed<NpcEquippedWeapon>,
            Changed<NpcWeaponMode>,
            Changed<NpcWeaponSlots>,
        )>,
    >,
) {
    for (entity, slots, equipped, weapon_mode, mut weapon) in &mut npcs {
        let item = equipped.map(|equipped| equipped.0);
        let drawn = weapon_mode.is_some_and(|mode| mode.0 == WeaponMode::Melee);
        // The joints are despawned with the visual when the npc is far
        let target_slot =
            if drawn { slots.hand } else { slots.hip }.filter(|slot| entities.contains(*slot));

        let path = item.and_then(|item| item_model_path(&gothic_scripts, &vfs, item));

        // The weapon mesh child is despawned with the visual when the npc is far
        let child_alive = weapon
            .as_ref()
            .and_then(|weapon| weapon.entity)
            .is_some_and(|child| entities.contains(child));

        let Some(path) = path else {
            // Nothing equipped, or its mesh is unknown
            detach_weapon(&mut commands, entities, weapon.as_deref_mut());
            continue;
        };

        if !child_alive {
            // Weapon child was despawned (npc went far) or never spawned
            let Some(target_slot) = target_slot else {
                // No slot available, only drop the stale child reference
                if let Some(mut weapon) = weapon {
                    weapon.entity = None;
                    weapon.slot = None;
                }
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
            attach_weapon(
                &mut commands,
                entity,
                weapon.as_deref_mut(),
                child,
                item,
                target_slot,
            );
            continue;
        }

        let Some(mut weapon) = weapon else {
            continue;
        };
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

/// Despawns the weapon mesh child when it is still alive and clears the tracked
/// state.
fn detach_weapon(commands: &mut Commands, entities: &Entities, weapon: Option<&mut NpcWeapon>) {
    let Some(weapon) = weapon else {
        return;
    };
    if let Some(child) = weapon.entity.take()
        && entities.contains(child)
    {
        commands.entity(child).despawn();
    }
    weapon.item = None;
    weapon.slot = None;
}

/// Records the spawned weapon mesh child, creating the component when it was
/// absent.
fn attach_weapon(
    commands: &mut Commands,
    entity: Entity,
    weapon: Option<&mut NpcWeapon>,
    child: Entity,
    item: Option<InstanceId>,
    slot: Entity,
) {
    match weapon {
        Some(weapon) => {
            weapon.entity = Some(child);
            weapon.item = item;
            weapon.slot = Some(slot);
        }
        None => {
            commands.entity(entity).insert(NpcWeapon {
                entity: Some(child),
                item,
                slot: Some(slot),
            });
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
pub(crate) fn overlay_scheme(name: &str) -> String {
    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    stem.to_uppercase()
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
/// script, hence this runs in `Update`.
fn resolve_goto_paths(
    mut changes: MessageReader<GotoChange>,
    index: Res<InstanceIndex>,
    gothic_script: Res<ScriptResource>,
    graph: Res<WaypointGraph>,
    mut commands: Commands,
    npcs: Query<&Transform>,
) {
    if changes.is_empty() {
        return;
    }

    let mut script = gothic_script.lock();
    for change in changes.read() {
        let Some(name) = script.get_name_by_id(change.waypoint) else {
            continue;
        };
        let Some(target) = graph.node_by_name(&name) else {
            continue;
        };
        for &entity in index.npcs(change.instance_id) {
            let Ok(transform) = npcs.get(entity) else {
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
