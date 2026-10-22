use bevy::{
    anti_alias::smaa::Smaa,
    camera_controller::free_camera::{FreeCamera, FreeCameraPlugin, FreeCameraState},
    input::mouse::AccumulatedMouseMotion,
    pbr::{ScreenSpaceAmbientOcclusion, ScreenSpaceAmbientOcclusionQualityLevel},
    prelude::*,
};

use crate::gameplay::character::PlayerPhysicsBodyMarker;
use crate::input::{GameplayAction, GameplayInputMarker};
use crate::xr::is_xr_enabled;
use crate::zengin::face_direction_quat;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use leafwing_input_manager::prelude::ActionState;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(FreeCameraPlugin);
        app.insert_resource(CameraResource::default());
        app.add_message::<TeleportToLocation>();
        app.add_systems(
            Update,
            (
                sync_free_camera_enabled,
                sync_third_person_cursor,
                (rotate_third_person_camera, rotate_third_person_camera_mouse)
                    .before(crate::gameplay::character_camera::camera_movement_system)
                    .run_if(use_third_person_camera),
            ),
        );
        app.add_systems(
            Update,
            update_dialog_camera
                .run_if(|info: Res<CameraResource>| info.current_camera == CurrentCamera::Dialog),
        );
        app.add_systems(Update, apply_teleport_requests);

        if !is_xr_enabled() {
            // Disable main camera for performance
            app.add_systems(Startup, spawn_camera);
        }
    }
}

#[derive(Resource, Default)]
pub struct CameraResource {
    current_camera: CurrentCamera,
    reference_point: Transform,
    /// Distance the dialog camera keeps from the reference point.
    dialog_distance: f32,
}

impl CameraResource {
    /// Controller that currently owns the camera transform.
    pub fn current(&self) -> CurrentCamera {
        self.current_camera
    }

    pub fn set_dialog_camera(&mut self, reference_point: Transform, distance: f32) {
        self.current_camera = CurrentCamera::Dialog;
        self.reference_point = reference_point;
        self.dialog_distance = distance;
    }

    /// Switches to the third person camera following the player.
    pub fn set_third_person_camera(&mut self) {
        self.current_camera = CurrentCamera::ThirdPerson;
    }

    /// Switches to the free camera, the player stops controlling it.
    pub fn set_free_camera(&mut self) {
        self.current_camera = CurrentCamera::Free;
    }
}
/// Which controller currently owns the camera transform. The free camera
/// controller is enabled only in `Free` mode and the cursor is locked and
/// hidden in `ThirdPerson`; otherwise the camera modes fight over the
/// transform.
#[derive(Default, PartialEq, Clone, Copy)]
pub enum CurrentCamera {
    #[default]
    Free,
    ThirdPerson,
    Dialog,
}

/// Rotation speed of the third person camera, radians per second.
const THIRD_PERSON_ROTATE_SPEED: f32 = 2.0;
/// Pitch limits of the third person camera, keeps the camera from flipping.
const THIRD_PERSON_PITCH_LIMIT: f32 = 1.4;
/// Rotation speed of the third person camera mouse movement, radians per
/// pixel of mouse movement.
const THIRD_PERSON_MOUSE_SENSITIVITY: f32 = 0.003;

struct DialogCameraStyle;
impl DialogCameraStyle {
    /// Minimum distance kept from the viewed character, even when the two talking
    /// characters stand very close to each other
    const MIN_DISTANCE: f32 = 2.0;
    const CAMERA_HEIGHT: f32 = 3.0;
    const LOOK_AT_HEIGHT: f32 = 1.45;
}

/// Saved location the debug UI can teleport the player and camera to.
pub struct TeleportLocation {
    pub name: &'static str,
    /// Ground position the player is moved to.
    pub player: Vec3,
    /// Camera position at the location.
    pub camera_position: Vec3,
    /// Point the camera looks at.
    pub camera_look_at: Vec3,
}

/// Saved locations, gathered from the spawn positions tried by hand.
pub const TELEPORT_LOCATIONS: &[TeleportLocation] = &[
    TeleportLocation {
        name: "Xardas Tower",
        player: Vec3::new(-297.4256, 51.482346, -159.4824),
        camera_position: Vec3::new(-299.0, 55.0, -154.5),
        camera_look_at: Vec3::new(-297.4256, 52.482346, -159.4824),
    },
    TeleportLocation {
        name: "Khorinis",
        player: Vec3::ZERO,
        camera_position: Vec3::new(40.0, 20.0, -10.0),
        camera_look_at: Vec3::ZERO,
    },
    TeleportLocation {
        name: "Khorinis Port",
        player: Vec3::new(-2.0, 0.0, -8.0),
        camera_position: Vec3::new(-3.0, 1.0, -3.0),
        camera_look_at: Vec3::new(-2.0, 0.0, -8.0),
    },
];

/// Teleports the player and camera to the saved location at `index` of
/// [`TELEPORT_LOCATIONS`], sent by the debug UI.
#[derive(Message)]
pub struct TeleportToLocation(pub usize);

fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Smaa::default(),
        Msaa::Off,
        // Msaa::Sample4,
        // TemporalAntiAliasing::default(),
        ScreenSpaceAmbientOcclusion {
            quality_level: ScreenSpaceAmbientOcclusionQualityLevel::Ultra,
            ..default()
        },
        Camera3d::default(),
        Transform::from_xyz(-3.0, 1.0, -3.0).looking_at(
            Vec3 {
                x: -2.0,
                y: 0.0,
                z: -8.0,
            },
            Vec3::Y,
        ),
        FreeCamera {
            sensitivity: 0.2,
            friction: 25.0,
            walk_speed: 3.0,
            run_speed: 9.0,
            // Colemak
            key_back: KeyCode::KeyR,
            key_right: KeyCode::KeyS,
            key_up: KeyCode::KeyF,
            ..default()
        },
    ));
}

/// Applies the teleport requests of the debug UI to the player body and the
/// camera.
fn apply_teleport_requests(
    mut teleports: MessageReader<TeleportToLocation>,
    mut player: Query<&mut Transform, (With<PlayerPhysicsBodyMarker>, Without<Camera3d>)>,
    mut camera: Query<&mut Transform, (With<Camera3d>, Without<PlayerPhysicsBodyMarker>)>,
) {
    for teleport in teleports.read() {
        let Some(location) = TELEPORT_LOCATIONS.get(teleport.0) else {
            warn!(
                "Teleport index {} is outside of TELEPORT_LOCATIONS, ignoring",
                teleport.0
            );
            continue;
        };
        if let Ok(mut transform) = camera.single_mut() {
            *transform = Transform::from_translation(location.camera_position)
                .looking_at(location.camera_look_at, Vec3::Y);
        } else {
            warn!("Teleport to {} failed: no camera found", location.name);
        }
        if let Ok(mut transform) = player.single_mut() {
            let direction =
                (location.camera_look_at - location.camera_position).normalize_or_zero();
            transform.translation = location.player;
            transform.rotation = face_direction_quat(direction);
        } else {
            warn!(
                "Teleport to {} failed: no player character found",
                location.name
            );
        }
    }
}

fn use_third_person_camera(info: Res<CameraResource>) -> bool {
    info.current_camera == CurrentCamera::ThirdPerson
}

fn sync_free_camera_enabled(
    info: Res<CameraResource>,
    mut query: Query<&mut FreeCameraState, With<Camera3d>>,
) {
    let Ok(mut state) = query.single_mut() else {
        return;
    };
    let enabled = info.current_camera == CurrentCamera::Free;
    if state.enabled != enabled {
        state.enabled = enabled;
    }
}
fn sync_third_person_cursor(
    info: Res<CameraResource>,
    mut windows: Query<(&mut Window, &mut CursorOptions), With<PrimaryWindow>>,
) {
    let Ok((_, mut cursor)) = windows.single_mut() else {
        return;
    };
    let third_person = use_third_person_camera(info);
    let locked = cursor.grab_mode == CursorGrabMode::Locked;
    if third_person == locked {
        return;
    }
    if third_person {
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
    } else {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    }
}

/// Rotates the third person camera with the `RotateCamera` action. The
/// camera position follows from its rotation in
/// [`crate::gameplay::character_camera::camera_movement_system`].
fn rotate_third_person_camera(
    time: Res<Time>,
    action_state: Single<&ActionState<GameplayAction>, With<GameplayInputMarker>>,
    mut camera_query: Query<&mut Transform, With<Camera3d>>,
) -> Result<(), BevyError> {
    let direction = action_state.axis_pair(&GameplayAction::RotateCamera);
    if direction.length_squared() < 0.01 {
        return Ok(());
    }

    let delta = THIRD_PERSON_ROTATE_SPEED * time.delta_secs();
    let mut transform = camera_query.single_mut()?;
    rotate_camera_transform(&mut transform, direction * delta);
    Ok(())
}

/// Rotates the third person camera with mouse movement.
fn rotate_third_person_camera_mouse(
    mouse_motion: Res<AccumulatedMouseMotion>,
    mut camera_query: Query<&mut Transform, With<Camera3d>>,
) -> Result<(), BevyError> {
    if mouse_motion.delta == Vec2::ZERO {
        return Ok(());
    }

    let mut transform = camera_query.single_mut()?;
    // TODO seems like y invert should be somewhere in input settings
    // Mouse y is inverted: moving the mouse up should look up. Side rotation
    // (x) keeps its direction.
    rotate_camera_transform(
        &mut transform,
        Vec2::new(mouse_motion.delta.x, -mouse_motion.delta.y) * THIRD_PERSON_MOUSE_SENSITIVITY,
    );
    Ok(())
}

/// Rotates the camera transform by `delta` (x: yaw, y: pitch in radians).
fn rotate_camera_transform(transform: &mut Transform, delta: Vec2) {
    let (yaw, pitch, _roll) = transform.rotation.to_euler(EulerRot::YXZ);
    // Looking right means rotating around Y by a negative angle.
    let yaw = yaw - delta.x;
    // Positive pitch looks up.
    let pitch = (pitch + delta.y).clamp(-THIRD_PERSON_PITCH_LIMIT, THIRD_PERSON_PITCH_LIMIT);
    transform.rotation = Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0);
}

pub(super) fn update_dialog_camera(
    info: ResMut<CameraResource>,
    mut camera_query: Query<&mut Transform, With<Camera3d>>,
) {
    let Ok(mut camera_transform) = camera_query.single_mut() else {
        return;
    };

    let npc_position = info.reference_point.translation;
    let npc_forward = *info.reference_point.forward();
    // The placement direction (behind the viewed character, offset to the side)
    // is carried by the reference rotation
    let npc_facing = -Vec3::new(npc_forward.x, 0.0, npc_forward.z).normalize_or_zero();

    let distance = info.dialog_distance.max(DialogCameraStyle::MIN_DISTANCE);
    let position =
        npc_position + npc_facing * distance + Vec3::Y * DialogCameraStyle::CAMERA_HEIGHT;
    camera_transform.translation = position;
    camera_transform.look_at(
        npc_position + Vec3::Y * DialogCameraStyle::LOOK_AT_HEIGHT,
        Vec3::Y,
    );
}
