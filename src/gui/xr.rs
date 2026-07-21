use bevy::render::pipelined_rendering::PipelinedRenderingPlugin;
use bevy::{prelude::*, window::PresentMode};
use bevy_egui::egui::mutex::Mutex;
use bevy_mod_openxr::{add_xr_plugins, resources::OxrSessionConfig};
use bevy_xr_utils::actions::XRUtilsActionsPlugin;
use openxr::EnvironmentBlendMode;
use std::cell::Cell;

use bevy_xr_utils::transform_utils::{SnapToPosition, TransformUtilitiesPlugin};
use std::sync::OnceLock;

pub fn is_xr_enabled() -> bool {
    static XR_ENABLED_STATIC: OnceLock<bool> = OnceLock::new();
    if let Some(xr_enabled) = XR_ENABLED_STATIC.get() {
        return *xr_enabled;
    }

    if let Ok(var) = std::env::var("ENABLE_XR") {
        if ["1", "y", "yes"].contains(&var.to_ascii_lowercase().as_str()) {
            println!("-- Bevy XR plugin enabled --");
            let _ = XR_ENABLED_STATIC.set(true);
            return true;
        }
    }
    let _ = XR_ENABLED_STATIC.set(false);
    return false;
}

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

        app.add_systems(Update, teleport);
        app.add_plugins(TransformUtilitiesPlugin);
        app.add_plugins(XRUtilsActionsPlugin);
    }
}

// TODO: movement
fn teleport(keys: Res<ButtonInput<KeyCode>>, mut event_writer: MessageWriter<SnapToPosition>) {
    if keys.just_pressed(KeyCode::Digit1) {
        event_writer.write(SnapToPosition(Vec3::new(0.0, 0.0, 0.0)));
    }
    if keys.just_pressed(KeyCode::Digit2) {
        event_writer.write(SnapToPosition(Vec3::new(10.0, 10.0, 10.0)));
    }
}
