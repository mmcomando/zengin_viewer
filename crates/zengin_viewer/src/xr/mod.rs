use bevy::log::info;
use bevy::render::pipelined_rendering::PipelinedRenderingPlugin;
use bevy::{prelude::*, window::PresentMode};
use bevy_egui::egui::mutex::Mutex;
use bevy_mod_openxr::{
    add_xr_plugins,
    helper_traits::ToQuat,
    openxr_session_running,
    resources::{OxrSessionConfig, OxrViews},
};
use bevy_mod_xr::session::XrTrackingRoot;
use bevy_xr_utils::actions::XRUtilsActionsPlugin;
use openxr::EnvironmentBlendMode;
use std::cell::Cell;

use bevy_xr_utils::transform_utils::{SnapToPosition, TransformUtilitiesPlugin};
use leafwing_input_manager::prelude::ActionState;
use std::sync::OnceLock;

use crate::input::actions::{DebugAction, DebugInputMarker, GameplayAction, GameplayInputMarker};

/// Experiment with VR
/// Worked with Pico 4 and SteamVR on Linux but very laggy (in the past I know it worked better)
/// With Pico 4 and WiVRn performance was smooth
pub struct XrPlugin {
    window_plugin_options: Mutex<Cell<WindowPlugin>>,
}

impl XrPlugin {
    pub fn new(window_plugin_options: WindowPlugin) -> Self {
        Self {
            window_plugin_options: Mutex::new(Cell::new(window_plugin_options)),
        }
    }
}

impl Plugin for XrPlugin {
    fn build(&self, app: &mut App) {
        let mut options = self.window_plugin_options.lock().take();
        // We want to sync to XR display
        options.primary_window.as_mut().unwrap().present_mode = PresentMode::AutoVsync;

        app.add_plugins(add_xr_plugins(
            // Disabling Pipelined Rendering should reduce latency a little bit for button inputs
            // and increase accuracy for hand tracking, controller positions and similar,
            // the views are updated right before rendering so they are as accurate as possible
            DefaultPlugins
                .set(options)
                .build()
                .disable::<PipelinedRenderingPlugin>(),
        ));
        app.insert_resource(OxrSessionConfig {
            blend_mode_preference: vec![
                EnvironmentBlendMode::ALPHA_BLEND,
                EnvironmentBlendMode::ADDITIVE,
                EnvironmentBlendMode::OPAQUE,
            ],
            ..default()
        });
        // app.insert_resource(ClearColor(Color::NONE)); // Uncomment so passthrough can be seen
        app.add_plugins(bevy_mod_xr::hand_debug_gizmos::HandGizmosPlugin);
        app.add_plugins(TransformUtilitiesPlugin);
        app.add_plugins(XRUtilsActionsPlugin);

        app.add_systems(
            Update,
            (teleport, move_camera).run_if(openxr_session_running),
        );
    }
}

pub fn is_xr_enabled() -> bool {
    static XR_ENABLED_STATIC: OnceLock<bool> = OnceLock::new();
    if let Some(xr_enabled) = XR_ENABLED_STATIC.get() {
        return *xr_enabled;
    }

    if let Ok(var) = std::env::var("ENABLE_XR")
        && ["1", "y", "yes"].contains(&var.to_ascii_lowercase().as_str())
    {
        info!("-- Bevy XR plugin enabled --");
        let _ = XR_ENABLED_STATIC.set(true);
        return true;
    }
    let _ = XR_ENABLED_STATIC.set(false);
    return false;
}

/// Smooth locomotion speed in m/s.
/// Matches the character controller's top speed (acceleration / damping = 35 / 10).
const XR_MOVE_SPEED: f32 = 3.5;

/// Moves the XR camera by moving the [`XrTrackingRoot`] with [`GameplayAction::Move`],
/// relative to the current head yaw.
fn move_camera(
    time: Res<Time>,
    action_state: Single<&ActionState<GameplayAction>, With<GameplayInputMarker>>,
    mut root: Single<&mut Transform, With<XrTrackingRoot>>,
    views: Res<OxrViews>,
) {
    // Keyboard D-pads and the gamepad stick are summed, so clamp to unit length.
    let direction = action_state
        .axis_pair(&GameplayAction::Move)
        .clamp_length_max(1.0);
    if direction.length_squared() < 0.1 {
        return;
    }
    let Some(view) = views.first() else {
        return;
    };

    // Use only the head yaw, so looking up or down doesn't change the walk speed
    let view_rotation = root.rotation * view.pose.orientation.to_quat();
    let yaw = view_rotation.to_euler(EulerRot::YXZ).0;
    let dir = Vec3 {
        x: direction.x,
        y: 0.0,
        z: -direction.y,
    }
    .rotate_y(yaw);

    root.translation += dir * XR_MOVE_SPEED * time.delta_secs();
}

fn teleport(
    action_state: Single<&ActionState<DebugAction>, With<DebugInputMarker>>,
    mut event_writer: MessageWriter<SnapToPosition>,
) {
    if action_state.just_pressed(&DebugAction::TeleportOrigin) {
        event_writer.write(SnapToPosition(Vec3::new(0.0, 0.0, 0.0)));
    }
    if action_state.just_pressed(&DebugAction::TeleportAway) {
        event_writer.write(SnapToPosition(Vec3::new(10.0, 10.0, 10.0)));
    }
}
