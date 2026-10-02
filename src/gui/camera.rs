use std::time::Duration;

// use bevy::prelude::*;

use bevy::{
    dev_tools::fps_overlay::{FpsOverlayConfig, FpsOverlayPlugin, FrameTimeGraphConfig},
    utils::default,
};

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
