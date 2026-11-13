//! Dialog camera: places the camera at one of the shots the original game
//! picks between. The shots are built and collision-checked once when a dialog
//! opens, then one is picked at random for every spoken line.

use super::state::DialogUiState;
use crate::gameplay::camera::{CameraResource, DialogShotPose};
use crate::zengin::STATIC_OBJECT;
use avian3d::prelude::{SpatialQuery, SpatialQueryFilter};
use bevy::prelude::*;
use rand_mt::Mt;

/// Height of the point the camera looks at, above a character's feet.
const LOOK_AT_HEIGHT: f32 = 1.45;
/// Height of the camera above a character's feet.
const CAMERA_HEIGHT: f32 = 1.7;
/// Safety margin subtracted from a camera collision distance so the near plane
/// does not touch the hit surface, same convention as the third-person camera.
const COLLISION_MARGIN: f32 = 0.3;
/// Smallest distance of the camera from the look-at point, keeps the camera
/// from ending up inside the viewed character when a shot is squeezed.
const MIN_DISTANCE: f32 = 0.5;

/// Camera shots, mirroring the four shots the original game picks between.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
enum DialogShot {
    Full = 0,
    OverTheShoulder = 1,
    Neutral = 2,
    CloseUp = 3,
}

const SHOT_COUNT: usize = 4;

const ALL_SHOTS: [DialogShot; SHOT_COUNT] = [
    DialogShot::Full,
    DialogShot::OverTheShoulder,
    DialogShot::Neutral,
    DialogShot::CloseUp,
];

/// Number of spoken lines before the close-up/neutral shots become available.
const SHOT_LIMIT: u32 = 2;

/// Shots cached for the current dialog, indexed by `[viewed_is_player][shot]`.
/// The character transforms are assumed to stay put for the whole dialog, so
/// the (camera position, look-at point) pairs are computed only on open.
#[derive(Resource)]
pub(super) struct DialogCameraShots {
    frames: [[Option<DialogShotPose>; SHOT_COUNT]; 2],
    /// Npc the cached frames were built for
    npc: Option<Entity>,
    /// Whether the camera system already saw the dialog open
    was_open: bool,
    /// Shot picking randomness, seeded per dialog so replays stay deterministic
    rng: Mt,
    previous_shot: DialogShot,
    shot_counter: u32,
    /// Line/option serial the applied shot was picked for
    applied_serial: Option<u64>,
}

impl Default for DialogCameraShots {
    fn default() -> Self {
        Self {
            frames: [[None; SHOT_COUNT]; 2],
            npc: None,
            was_open: false,
            // Replaced with a per-dialog seed on open
            rng: Mt::new(0),
            previous_shot: DialogShot::Full,
            shot_counter: 0,
            applied_serial: None,
        }
    }
}

/// Builds the shot frames when a dialog opens and picks a random shot for every
/// spoken line, reusing the cached geometry.
pub(super) fn aim_dialog_camera(
    dialog_ui: Res<DialogUiState>,
    mut shots: ResMut<DialogCameraShots>,
    mut camera_info: ResMut<CameraResource>,
    transforms: Query<&GlobalTransform>,
    spatial_query: SpatialQuery,
) {
    if !dialog_ui.is_open() {
        shots.was_open = false;
        return;
    }
    let (Some(npc), Some(player)) = (dialog_ui.npc_entity, dialog_ui.player_entity) else {
        return;
    };
    let (Ok(npc_tr), Ok(player_tr)) = (transforms.get(npc), transforms.get(player)) else {
        return;
    };

    // The dialog open handler turns the characters to face each other before
    // this system runs, so the positions are final for the whole dialog.
    if !shots.was_open || shots.npc != Some(npc) {
        let seed = dialog_seed(&dialog_ui);
        rebuild(
            &mut shots,
            npc,
            npc_tr.translation(),
            player_tr.translation(),
            seed,
            &spatial_query,
        );
        shots.was_open = true;
    }

    let viewed_is_player = dialog_ui
        .camera_characters()
        .is_some_and(|(viewed, _)| viewed == player);
    let serial = dialog_ui.camera_shot_serial();
    if shots.applied_serial == Some(serial) {
        return;
    }
    shots.applied_serial = Some(serial);

    let shot = shots.pick_shot(viewed_is_player);
    let Some(pose) = shots.frames[usize::from(viewed_is_player)][shot as usize] else {
        return;
    };
    camera_info.set_dialog_shot(pose);
}

/// Seed of the per-dialog shot randomness, derived from the two script
/// instances so the same dialog always produces the same shot sequence.
fn dialog_seed(dialog_ui: &DialogUiState) -> u32 {
    let npc = dialog_ui.npc_talk_to.map_or(0, |id| id.value() as u32);
    let player = dialog_ui.npc_player.map_or(0, |id| id.value() as u32);
    (npc ^ player.rotate_left(16)) ^ 0x9E37_79B9
}

/// Computes and collision-checks the frames for both speaker directions.
fn rebuild(
    shots: &mut DialogCameraShots,
    npc: Entity,
    npc_pos: Vec3,
    player_pos: Vec3,
    seed: u32,
    spatial_query: &SpatialQuery,
) {
    shots.frames = build_frames(npc_pos, player_pos, spatial_query);
    shots.npc = Some(npc);
    shots.rng = Mt::new(seed);
    shots.previous_shot = DialogShot::Full;
    shots.shot_counter = 0;
    shots.applied_serial = None;
}

/// Builds all shots for both speaker directions, shortening every camera
/// position that would end up behind static world geometry.
fn build_frames(
    npc_pos: Vec3,
    player_pos: Vec3,
    spatial_query: &SpatialQuery,
) -> [[Option<DialogShotPose>; SHOT_COUNT]; 2] {
    let up = Vec3::Y;
    let distance = npc_pos.distance(player_pos).max(MIN_DISTANCE);
    let npc_facing = horizontal_direction(player_pos - npc_pos);
    let midpoint = (npc_pos + player_pos) * 0.5;
    // The neutral shot always sits on the same side of the npc, regardless of
    // who speaks, so it does not mirror when the speaker changes.
    let neutral_side = up.cross(npc_facing).normalize_or_zero();

    let mut frames = [[None; SHOT_COUNT]; 2];
    for (index, (viewed, listener, reverse)) in [
        (npc_pos, player_pos, 1.0f32),
        (player_pos, npc_pos, -1.0f32),
    ]
    .into_iter()
    .enumerate()
    {
        let facing = horizontal_direction(listener - viewed);
        let right = up.cross(facing).normalize_or_zero();
        let look_at = viewed + up * LOOK_AT_HEIGHT;

        frames[index][DialogShot::Full as usize] = Some(collide(
            DialogShotPose {
                camera: viewed
                    + facing * (distance + 0.3)
                    + right * (-1.5 * reverse)
                    + up * CAMERA_HEIGHT,
                look_at,
            },
            spatial_query,
        ));
        frames[index][DialogShot::OverTheShoulder as usize] = Some(collide(
            DialogShotPose {
                camera: listener + facing * 0.6 + right * (0.5 * reverse) + up * CAMERA_HEIGHT,
                look_at,
            },
            spatial_query,
        ));
        frames[index][DialogShot::CloseUp as usize] = Some(collide(
            DialogShotPose {
                camera: viewed + facing * 1.5 + right * (0.3 * reverse) + up * CAMERA_HEIGHT,
                look_at,
            },
            spatial_query,
        ));
    }

    // The neutral shot is a side view of the pair, the same for both directions
    let neutral = collide(
        DialogShotPose {
            camera: midpoint - neutral_side * distance + up * CAMERA_HEIGHT,
            look_at: midpoint + up * LOOK_AT_HEIGHT,
        },
        spatial_query,
    );
    frames[0][DialogShot::Neutral as usize] = Some(neutral);
    frames[1][DialogShot::Neutral as usize] = Some(neutral);

    frames
}

/// Horizontal unit direction of `direction`, `+Z` when it is zero.
fn horizontal_direction(direction: Vec3) -> Vec3 {
    let horizontal = Vec3::new(direction.x, 0.0, direction.z);
    horizontal.try_normalize().unwrap_or(Vec3::Z)
}

/// Shortens the camera position along the look-at ray when static geometry is
/// in the way, leaving the look-at point untouched.
fn collide(pose: DialogShotPose, spatial_query: &SpatialQuery) -> DialogShotPose {
    let offset = pose.camera - pose.look_at;
    let Some(direction) = Dir3::new(offset).ok() else {
        return pose;
    };
    let desired = offset.length();
    if desired <= MIN_DISTANCE {
        return pose;
    }
    let Some(hit) = spatial_query.cast_ray(
        pose.look_at,
        direction,
        desired,
        false,
        &SpatialQueryFilter::from_mask(STATIC_OBJECT),
    ) else {
        return pose;
    };
    let distance = (hit.distance - COLLISION_MARGIN).clamp(MIN_DISTANCE, desired);
    let direction_vec = *direction;
    DialogShotPose {
        camera: pose.look_at + direction_vec * distance,
        look_at: pose.look_at,
    }
}

impl DialogCameraShots {
    /// Picks the next shot, following the original game's progression: the
    /// first lines and player lines only use the wide shots, the close-up can
    /// only follow a neutral shot.
    fn pick_shot(&mut self, viewed_is_player: bool) -> DialogShot {
        let mut next = self.previous_shot;
        if viewed_is_player || self.shot_counter <= SHOT_LIMIT {
            if self.shot_counter == 0 || self.shot_counter > SHOT_LIMIT {
                next = if self.rng.next_u32().is_multiple_of(2) {
                    DialogShot::Full
                } else {
                    DialogShot::OverTheShoulder
                };
            }
        } else if self.previous_shot == DialogShot::Neutral
            && self.rng.next_u32().is_multiple_of(SHOT_COUNT as u32)
        {
            next = DialogShot::CloseUp;
        } else if self.previous_shot != DialogShot::CloseUp {
            next = ALL_SHOTS[self.rng.next_u32() as usize % SHOT_COUNT];
        }
        self.previous_shot = next;
        if viewed_is_player {
            self.shot_counter = 0;
        } else {
            self.shot_counter += 1;
        }
        next
    }
}
