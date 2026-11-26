mod egui;
mod entity_details;
mod entity_list;
mod gizmos;
mod info_tab;
mod state;
mod statistics;
mod toggle_visibility;

use std::time::Duration;

use bevy::dev_tools::fps_overlay::{FpsOverlayConfig, FpsOverlayPlugin, FrameTimeGraphConfig};
use bevy::prelude::*;

use crate::gui_debug::egui::GameDebugUI;
use crate::gui_debug::gizmos::GizmosPlugin;
pub(crate) use crate::gui_debug::statistics::{RefreshStatistics, StatisticsPlugin};
use crate::gui_debug::toggle_visibility::ToggleVisibility;
use crate::input::Options;

/// Adds all debug UI plugins (visibility toggles, egui debug window, gizmos,
/// statistics, FPS overlay). The debug UI can be disabled with `--no-debug-gui`,
/// and the FPS overlay is skipped in renderless mode.
pub struct GuiDebugPlugin;

impl Plugin for GuiDebugPlugin {
    fn build(&self, app: &mut App) {
        let no_debug_gui = app.world().resource::<Options>().no_debug_gui();
        let renderless = app.world().resource::<Options>().renderless();

        app.add_plugins(ToggleVisibility);
        if !no_debug_gui {
            app.add_plugins(GameDebugUI).add_plugins(GizmosPlugin);
        }
        app.add_plugins(StatisticsPlugin);
        if !renderless && !no_debug_gui {
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
