use std::collections::HashMap;

use bevy::prelude::*;

use super::time::GameTime;
use crate::game::objects::GameNpc;
use crate::zengin::script::component::WorldScript;

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

#[derive(Resource, Default)]
pub(super) struct NpcRoutineCheckState {
    last_checked_hour: Option<u32>,
}

pub(super) fn add_systems(app: &mut App) {
    app.init_resource::<NpcRoutineCheckState>();
    app.add_systems(
        Update,
        update_npc_routines.after(super::time::advance_game_time),
    );
}

/// Periodically (on every in-game hour change) checks the active routine entry of each npc.
/// When the routine waypoint changed, the npc is teleported to the new waypoint location.
fn update_npc_routines(
    mut check_state: ResMut<NpcRoutineCheckState>,
    game_time: Res<GameTime>,
    gothic_script: Res<WorldScript>,
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
    let hour = game_time.hour();
    if check_state.last_checked_hour == Some(hour) {
        return;
    }
    let first_check = check_state.last_checked_hour.is_none();
    check_state.last_checked_hour = Some(hour);

    let loaded_state = gothic_script.get_loaded_state();

    for (entity, npc, mut tr, routine_state) in &mut npcs {
        let Some(instance) = loaded_state
            .instance_data
            .get(&(npc.npc_instance_id as u32))
        else {
            continue;
        };
        let Some(entry) = instance.get_routine_entry(hour) else {
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
