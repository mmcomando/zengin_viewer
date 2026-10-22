mod egui;
mod gizmos;
mod statistics;
mod toggle_visibility;

use std::time::Duration;

use bevy::dev_tools::fps_overlay::{FpsOverlayConfig, FpsOverlayPlugin, FrameTimeGraphConfig};
use bevy::prelude::*;

use crate::gui_debug::egui::GameDebugUI;
use crate::gui_debug::gizmos::GizmosPlugin;
use crate::gui_debug::statistics::StatisticsPlugin;
use crate::gui_debug::toggle_visibility::ToggleVisibility;
use crate::input::Options;

pub use crate::gui_debug::toggle_visibility::{NpcVisibility, StaticMesh, WorldMesh};

/// Adds all debug UI plugins (visibility toggles, egui debug window, gizmos,
/// statistics, FPS overlay). Configuration comes from the `Options` resource:
/// scenario replays get no debug overlays so screenshots stay clean, and the
/// FPS overlay is skipped in renderless mode.
pub struct GuiDebugPlugin;

impl Plugin for GuiDebugPlugin {
    fn build(&self, app: &mut App) {
        // TODO Add option in options.rs to completely disable this plugin
        let overlays_hidden = app.world().resource::<Options>().replay_actions().is_some();
        let renderless = app.world().resource::<Options>().renderless();

        app.add_plugins(ToggleVisibility);
        if !overlays_hidden {
            app.add_plugins(GameDebugUI).add_plugins(GizmosPlugin);
        }
        app.add_plugins(StatisticsPlugin);
        if !renderless && !overlays_hidden {
            app.add_plugins(fps_overlay_plugin());
        }
    }
}

/// FPS overlay for debugging, requires rendering.
fn fps_overlay_plugin() -> FpsOverlayPlugin {
    FpsOverlayPlugin {
        config: FpsOverlayConfig {
            enabled: true,
            refresh_interval: Duration::from_millis(500),
            frame_time_graph_config: FrameTimeGraphConfig {
                enabled: true,
                min_fps: 30.0,
                target_fps: 144.0,
            },
            ..default()
        },
    }
}
