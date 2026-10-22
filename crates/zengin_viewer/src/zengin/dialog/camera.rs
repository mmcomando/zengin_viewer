//! Dialog camera: places the camera behind the listening character during a
//! dialog.

use super::state::DialogUiState;
use crate::gameplay::camera::CameraResource;
use bevy::prelude::*;

/// How far the camera is offset to the side from directly behind the listening
/// character, as a fraction of the character separation. The side is fixed to
/// the pair of characters so the shot does not mirror when the speaker changes.
const DIALOG_CAMERA_SIDE: f32 = 0.6;

/// Places the dialog camera behind the character that is currently listening,
/// looking back at the speaker, with its distance scaled to how far the two
/// talking characters stand apart.
pub(super) fn aim_dialog_camera(
    dialog_ui: Res<DialogUiState>,
    mut camera_info: ResMut<CameraResource>,
    transforms: Query<&GlobalTransform>,
) {
    if !dialog_ui.is_open() {
        return;
    }
    let (Some((viewed, other)), Some(player), Some(npc)) = (
        dialog_ui.camera_characters(),
        dialog_ui.player_entity,
        dialog_ui.npc_entity,
    ) else {
        return;
    };
    let (Ok(viewed_tr), Ok(other_tr), Ok(player_tr), Ok(npc_tr)) = (
        transforms.get(viewed),
        transforms.get(other),
        transforms.get(player),
        transforms.get(npc),
    ) else {
        return;
    };

    let viewed_pos = viewed_tr.translation();
    let other_pos = other_tr.translation();
    let separation = viewed_pos.distance(other_pos);
    // Behind the listening character is away from the speaking one, offset to a
    // fixed side of the pair so the shot does not mirror when the speaker changes
    let behind = (other_pos - viewed_pos).normalize_or_zero();
    let pair = (player_tr.translation() - npc_tr.translation()).normalize_or_zero();
    let side = Vec3::Y.cross(pair).normalize_or_zero();
    let behind = (behind + side * DIALOG_CAMERA_SIDE).normalize_or_zero();
    // The camera sits behind the listener and looks back at the speaker (`viewed`)
    let camera_pos = other_pos + behind * separation;
    let to_camera = Vec3::new(
        camera_pos.x - viewed_pos.x,
        0.0,
        camera_pos.z - viewed_pos.z,
    );
    let distance = to_camera.length();

    let reference = Transform::from_translation(viewed_pos)
        .with_rotation(Quat::from_rotation_y(to_camera.x.atan2(to_camera.z)));
    camera_info.set_dialog_camera(reference, distance);
}
