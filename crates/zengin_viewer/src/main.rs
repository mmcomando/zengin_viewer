#![allow(
    clippy::needless_pass_by_value,
    clippy::cast_possible_truncation,
    clippy::uninlined_format_args,
    clippy::needless_return,
    clippy::enum_variant_names,
    clippy::too_many_lines,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::manual_assert,
    clippy::case_sensitive_file_extension_comparisons,
    clippy::wildcard_imports
)]

mod camera;
mod character;
mod game;
mod gui;
mod input;
mod skybox;
mod zengin;

use crate::camera::CameraPlugin;
use crate::character::CharacterControllerPlugin;
use crate::game::animation::SkeletalAnimations;
use crate::game::audio::GameAudioPlugin;
use crate::game::disco_lamp::DiscoLamp;
use crate::game::distance::DistancePlugin;
use crate::game::game_time::GameTimePlugin;
use crate::game::lighting::LightingPlugin;
use crate::gui::camera::get_overlay_plugin;
use crate::gui::egui::GameDebugUI;
use crate::gui::statistics::StatisticsPlugin;
use crate::gui::toggle_visibility::ToggleVisibility;
use crate::gui::xr::{XrPlugin, is_xr_enabled};
use crate::input::InputPlugin;
use crate::skybox::SkyBoxPlugin;
use crate::zengin::SpeechSounds;
use crate::zengin::ZenginPlugin;
use crate::zengin::{create_zengin_asset_loader, create_zengin_vfs};
use avian3d::prelude::*;

use bevy::prelude::*;
use bevy::window::PresentMode;

fn main() {
    let mut app = App::new();
    // This app custom assert source, has to be registered before default plugins.
    let vfs = create_zengin_vfs();
    app.insert_resource(SpeechSounds::new(vfs.clone()));
    app.register_asset_source("zengin", create_zengin_asset_loader(vfs));
    // Default plugin
    let window_plugin_options = WindowPlugin {
        primary_window: Some(Window {
            title: "ZenGin Walker".into(),
            present_mode: PresentMode::AutoNoVsync,
            // present_mode: PresentMode::AutoVsync,
            ..default()
        }),
        ..default()
    };
    // XR or standard rendering
    if is_xr_enabled() {
        app.add_plugins(XrPlugin::new(window_plugin_options));
    } else {
        app.add_plugins(DefaultPlugins.set(window_plugin_options));
    }

    // Plugins from crates
    app.add_plugins(PhysicsPlugins::default())
        .add_plugins(get_overlay_plugin());

    // This app custom plugins
    app.add_plugins(InputPlugin)
        .add_plugins(ZenginPlugin)
        .add_plugins(SkyBoxPlugin)
        .add_plugins(ToggleVisibility)
        .add_plugins(CharacterControllerPlugin)
        .add_plugins(DiscoLamp)
        .add_plugins(DistancePlugin)
        .add_plugins(GameTimePlugin)
        .add_plugins(LightingPlugin)
        .add_plugins(GameDebugUI)
        .add_plugins(StatisticsPlugin)
        .add_plugins(GameAudioPlugin)
        .add_plugins(CameraPlugin)
        .add_plugins(SkeletalAnimations);
    // Run
    app.run();
}

// Shadow maps are very heavy and disco demo uses 4 of them tanking performance
// Enabling this make fps go from 300 to 80
const ENABLE_DISCO_SHADOW_MAPS: bool = false;
