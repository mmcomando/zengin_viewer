//! Third-person camera toggle and camera movement for the player character.

use avian3d::prelude::*;
use bevy::prelude::*;
use leafwing_input_manager::prelude::ActionState;

use crate::gameplay::camera::{CameraResource, CurrentCamera, EditorMode};
use crate::gameplay::character::PLAYER_MODEL_ROOT_TO_CAPSULE_CENTER;
use crate::input::{CameraSwitchedToThirdPerson, EditorAction, EditorInputMarker};
use crate::zengin::PlayerCharacter;
use crate::zengin::STATIC_OBJECT;
use dev_util::perf_debug;

pub(crate) fn use_character_camera(info: Res<CameraResource>) -> bool {
    info.current() == CurrentCamera::ThirdPerson
}

/// Toggles [`EditorMode`]. Leaving editor mode places the player at the free
/// camera position so gameplay continues where the camera was looking.
#[allow(clippy::type_complexity)]
pub(crate) fn camera_toggle_input(
    mut editor_mode: ResMut<EditorMode>,
    action_state: Single<&ActionState<EditorAction>, With<EditorInputMarker>>,
    camera_query: Query<&GlobalTransform, (With<Camera3d>, Without<PlayerCharacter>)>,
    mut player: Query<&mut Transform, (With<PlayerCharacter>, Without<Camera3d>)>,
    mut switches: MessageWriter<CameraSwitchedToThirdPerson>,
) {
    let _perf = perf_debug!();
    if !action_state.just_pressed(&EditorAction::ToggleEditorMode) {
        return;
    }
    editor_mode.toggle();
    if editor_mode.is_enabled() {
        return;
    }
    // Player starts where the free camera was looking
    if let (Ok(camera_transform), Ok(mut player_transform)) =
        (camera_query.single(), player.single_mut())
    {
        place_player_at_free_camera(camera_transform, &mut player_transform);
        switches.write(CameraSwitchedToThirdPerson(*camera_transform));
    }
}

/// Places the player at the free camera position, facing the camera's look
/// direction.
fn place_player_at_free_camera(
    camera_transform: &GlobalTransform,
    player_transform: &mut Transform,
) {
    player_transform.translation = camera_transform.translation();
    let forward = camera_transform.forward();
    let dir = Vec3::new(forward.x, 0.0, forward.z);
    if dir.length_squared() > f32::EPSILON {
        let dir = dir.normalize();
        player_transform.rotation = Quat::from_rotation_y(dir.x.atan2(dir.z));
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
/// Height of the camera pivot above the capsule center.
const CAM_PIVOT_BODY_HEIGHT: f32 = 1.3;
/// Height of the camera pivot above the player entity origin (model root).
const CAM_PIVOT_HEIGHT: f32 = CAM_PIVOT_BODY_HEIGHT + PLAYER_MODEL_ROOT_TO_CAPSULE_CENTER;

#[allow(clippy::type_complexity)]
pub fn camera_movement_system(
    player_query: Single<(&Transform, Entity), (With<PlayerCharacter>, Without<Camera3d>)>,
    mut camera_transform: Single<&mut Transform, (With<Camera3d>, Without<PlayerCharacter>)>,
    spatial_query: SpatialQuery,
) {
    let _perf = perf_debug!();
    let (player_transform, player_entity) = *player_query;

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
