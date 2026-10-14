use std::collections::HashMap;

use bevy::prelude::*;

use crate::gameplay::game_time::{GameTime, advance_game_time};
use crate::gameplay::objects::NpcModel;
use crate::graphics::animation::AnimationController;
use crate::zengin::GameNpc;
use crate::zengin::common::get_animation_path;
use crate::zengin::script::component::ScriptResource;
use crate::zengin::script::symbol_id::FunctionId;
use crate::zengin::zengin_resources::MaterialHandles;

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
                update_npc_routine_animations.after(update_npc_routines),
            )
                .after(advance_game_time),
        );
    }
}

/// Periodically (on every in-game minute change) checks the active routine
/// entry of each npc. When the routine waypoint changed, the npc is teleported
/// to the new waypoint location.
fn update_npc_routines(
    mut check_state: ResMut<NpcRoutineCheckState>,
    game_time: Res<GameTime>,
    gothic_script: Res<ScriptResource>,
    way_points: Res<WayPoints>,
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
                debug!(
                    "teleport npc({}) to waypoint({})",
                    npc.npc_instance_id, entry.way_point
                );
                *tr = Transform::from_translation(
                    waypoint_tr.translation + Vec3::Y * NPC_BODY_Y_OFFSET,
                )
                .with_rotation(waypoint_tr.rotation);
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
    mut handles: ResMut<MaterialHandles>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
    mut npcs: Query<
        (
            Entity,
            &GameNpc,
            &NpcModel,
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

    for (entity, npc, npc_model, routine_animation, controller) in &mut npcs {
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
