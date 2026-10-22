//! Third-person camera toggle and camera movement for the player character.

use avian3d::prelude::*;
use bevy::prelude::*;
use leafwing_input_manager::prelude::ActionState;

use crate::gameplay::camera::{CameraResource, CurrentCamera};
use crate::gameplay::character::PlayerPhysicsBodyMarker;
use crate::gameplay::objects::PlayerCharacter;
use crate::input::actions::{DebugAction, DebugInputMarker};
use crate::zengin::STATIC_OBJECT;

pub(crate) fn use_character_camera(info: Res<CameraResource>) -> bool {
    info.current_camera == CurrentCamera::ThirdPerson
}

/// Toggles between the free camera and the character camera.
pub(crate) fn camera_toggle_input(
    mut info: ResMut<CameraResource>,
    action_state: Single<&ActionState<DebugAction>, With<DebugInputMarker>>,
    camera_query: Query<&GlobalTransform, (With<Camera3d>, Without<PlayerPhysicsBodyMarker>)>,
    mut player_query: Query<&mut Transform, With<PlayerPhysicsBodyMarker>>,
    mut player_visual: Query<
        &mut Transform,
        (
            With<PlayerCharacter>,
            Without<PlayerPhysicsBodyMarker>,
            Without<Camera3d>,
        ),
    >,
) {
    // TODO Make it early return !action_state {return}
    if action_state.just_pressed(&DebugAction::ToggleCharacterCamera) {
        let next_camera = match info.current_camera {
            CurrentCamera::Free => CurrentCamera::ThirdPerson,
            CurrentCamera::ThirdPerson => CurrentCamera::Free,
            CurrentCamera::Dialog => CurrentCamera::Free,
        };
        if next_camera == CurrentCamera::ThirdPerson {
            // Move this branch to separate function, it is separate self contained logic which can be documented by new function name
            // Player starts where the free camera was looking
            if let (Ok(camera_transform), Ok(mut player_transform)) =
                (camera_query.single(), player_query.single_mut())
            {
                player_transform.translation = camera_transform.translation();
                // The camera looks along its forward; make the character face
                // that direction (the model carries the facing, not the body).
                // TODO after move to new function there could be early return
                if let Ok(mut visual_transform) = player_visual.single_mut() {
                    let forward = camera_transform.forward();
                    let dir = Vec3::new(forward.x, 0.0, forward.z);
                    if dir.length_squared() > f32::EPSILON {
                        let dir = dir.normalize();
                        visual_transform.rotation = Quat::from_rotation_y(dir.x.atan2(dir.z));
                    }
                }
            }
        }
        info.current_camera = next_camera;
    }
}

/// Maximum distance of the third person camera from the player.
const MAX_CAM_DISTANCE: f32 = 6.0;
/// Safety margin subtracted from the camera collision distance so the near
/// plane does not touch the hit surface.
const CAM_COLLISION_MARGIN: f32 = 0.3;
/// Smallest allowed camera distance, keeps the camera from reaching the pivot
/// when the player is very close to geometry.
const MIN_CAM_DISTANCE: f32 = 0.5;
/// Height of the camera pivot above the player origin.
const CAM_PIVOT_HEIGHT: f32 = 1.3;

pub fn camera_movement_system(
    player_query: Single<(&Transform, Entity), (With<PlayerPhysicsBodyMarker>, Without<Camera3d>)>,
    mut camera_query: Query<&mut Transform, (With<Camera3d>, Without<PlayerPhysicsBodyMarker>)>,
    spatial_query: SpatialQuery,
) {
    let (player_transform, player_entity) = *player_query;
    // TODO use the Single system param instead of single_mut().unwrap()
    let mut camera_transform = camera_query.single_mut().unwrap();

    let offset_direction = camera_transform.rotation * Vec3::Z;
    let pivot = player_transform.translation + Vec3::Y * CAM_PIVOT_HEIGHT;
    let Ok(ray_direction) = Dir3::new(offset_direction) else {
        warn!(
            "Third person camera offset direction is not a valid unit direction: {:?}",
            offset_direction
        );
        return;
    };

    // Cast a ray from the pivot towards the camera position and clamp the
    // distance so the camera never goes behind static world geometry.
    let distance = match spatial_query.cast_ray(
        pivot,
        ray_direction,
        MAX_CAM_DISTANCE,
        false,
        &SpatialQueryFilter::from_excluded_entities([player_entity]).with_mask(STATIC_OBJECT),
    ) {
        Some(hit) => (hit.distance - CAM_COLLISION_MARGIN).max(MIN_CAM_DISTANCE),
        None => MAX_CAM_DISTANCE,
    };

    camera_transform.translation = pivot + offset_direction * distance;
}
