use bevy::{
    anti_alias::smaa::Smaa,
    camera_controller::free_camera::{FreeCamera, FreeCameraPlugin},
    dev_tools::fps_overlay::{FpsOverlayConfig, FpsOverlayPlugin, FrameTimeGraphConfig},
    pbr::{ContactShadows, ScreenSpaceAmbientOcclusion, ScreenSpaceAmbientOcclusionQualityLevel},
    prelude::*,
};
use std::time::Duration;

use crate::xr::is_xr_enabled;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(FreeCameraPlugin);
        app.insert_resource(CameraResource::default());
        app.add_systems(Update, update_dialog_camera.run_if(use_dialog_camera));

        // Currently ThirdPerson relies on FreeCamera to change orientation of Camera
        // TODO disable FreeCamera when other camera type is enabled
        // app.add_systems(Update, sync_free_camera_enabled);

        if !is_xr_enabled() {
            // Disable main camera for performance
            app.add_systems(Startup, spawn_camera);
        }
    }
}

#[derive(Resource, Default)]
pub struct CameraResource {
    pub current_camera: CurrentCamera,
    pub reference_point: Transform,
    /// Distance the dialog camera keeps from `reference_point`, set to the
    /// distance between the two talking characters.
    pub dialog_distance: f32,
}

impl CameraResource {
    pub fn set_dialog_camera(&mut self, reference_point: Transform, distance: f32) {
        self.current_camera = CurrentCamera::Dialog;
        self.reference_point = reference_point;
        self.dialog_distance = distance;
    }
}
#[derive(Default, PartialEq)]
pub enum CurrentCamera {
    #[default]
    Free,
    ThirdPerson,
    Dialog,
}

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
        // Contact shadows introduce visible noise on models, disable it until it improves
        ContactShadows::default(),
        // Xardas
        // Transform::from_xyz(-297.0 - 2.0, 55.0, -159.5 + 5.0).looking_at(
        //     Vec3 {
        //         x: -297.4256,
        //         y: 51.482346 + 1.0,
        //         z: -159.4824,
        //     },
        //     Vec3::Y,
        // ),
        // Khorinis
        // Transform::from_xyz(40.0, 20.0, -10.0).looking_at(
        //     Vec3 {
        //         x: 0.0,
        //         y: 0.0,
        //         z: 0.0,
        //     },
        //     Vec3::Y,
        // ),
        // Khorinis Close Look at 0,1,0
        Transform::from_xyz(2.0, 1.0, 4.0).looking_at(
            Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
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

fn use_dialog_camera(info: Res<CameraResource>) -> bool {
    info.current_camera == CurrentCamera::Dialog
}

// /// Keeps bevy's free camera controller enabled only in the free camera mode,
// /// otherwise it fights with the other camera systems for the transform
// fn sync_free_camera_enabled(
//     info: Res<CameraResource>,
//     mut query: Query<&mut FreeCameraState, With<Camera3d>>,
// ) {
//     let Ok(mut state) = query.single_mut() else {
//         return;
//     };
//     let enabled = info.current_camera == CurrentCamera::Free;
//     if state.enabled != enabled {
//         state.enabled = enabled;
//     }
// }

struct DialogCameraStyle;
impl DialogCameraStyle {
    /// Minimum distance kept from the viewed character, even when the two talking
    /// characters stand very close to each other
    const MIN_DISTANCE: f32 = 2.0;
    const CAMERA_HEIGHT: f32 = 3.0;
    const LOOK_AT_HEIGHT: f32 = 1.45;
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

pub fn get_overlay_plugin() -> FpsOverlayPlugin {
    FpsOverlayPlugin {
        config: FpsOverlayConfig {
            enabled: true,
            refresh_interval: Duration::from_millis(500),
            frame_time_graph_config: FrameTimeGraphConfig {
                enabled: true,
                // The minimum acceptable fps
                min_fps: 30.0,
                // The target fps
                target_fps: 144.0,
            },
            ..default()
        },
    }
}
