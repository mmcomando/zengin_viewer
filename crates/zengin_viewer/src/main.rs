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

mod audio;
mod gameplay;
mod graphics;
mod gui_debug;
mod input;
mod xr;
mod zengin;

use crate::audio::sound_manager::GameAudioPlugin;
use crate::gameplay::camera::{CameraPlugin, get_overlay_plugin};
use crate::gameplay::character::CharacterControllerPlugin;
use crate::gameplay::distance::DistancePlugin;
use crate::gameplay::game_time::GameTimePlugin;
use crate::graphics::animation::SkeletalAnimations;
use crate::graphics::disco_lamp::DiscoLamp;
use crate::graphics::lighting::LightingPlugin;
use crate::graphics::render_options::RenderOptionsPlugin;
use crate::graphics::skybox::SkyBoxPlugin;
use crate::gui_debug::egui::GameDebugUI;
use crate::gui_debug::statistics::StatisticsPlugin;
use crate::gui_debug::toggle_visibility::ToggleVisibility;
use crate::input::actions::ActionsPlugin;
use crate::input::options::Options;
use crate::xr::{XrPlugin, is_xr_enabled};
use crate::zengin::SpeechSounds;
use crate::zengin::ZenginPlugin;
use crate::zengin::{create_zengin_asset_loader, create_zengin_vfs};
use avian3d::prelude::*;

use bevy::prelude::*;
use bevy::window::PresentMode;

fn main() {
    let mut app = App::new();
    // Command line options, scenario handling has to be registered first.
    app.add_plugins(crate::input::options::OptionsPlugin);
    // This app custom assert source, has to be registered before default plugins.
    let vfs = create_zengin_vfs();
    app.insert_resource(SpeechSounds::new(vfs.clone()));
    app.register_asset_source("zengin", create_zengin_asset_loader(vfs));
    // Default plugin
    let mut primary_window = Window {
        title: "ZenGin Walker".into(),
        present_mode: PresentMode::AutoNoVsync,
        // present_mode: PresentMode::AutoVsync,
        ..default()
    };
    let scenario = app.world().resource::<Options>().scenario();
    crate::input::scenario::apply_scenario_window_settings(&mut primary_window, scenario);
    let window_plugin_options = WindowPlugin {
        primary_window: Some(primary_window),
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
    app.add_plugins(ActionsPlugin)
        .add_plugins(ZenginPlugin)
        .add_plugins(SkyBoxPlugin)
        .add_plugins(ToggleVisibility)
        .add_plugins(CharacterControllerPlugin)
        .add_plugins(DiscoLamp)
        .add_plugins(DistancePlugin)
        .add_plugins(GameTimePlugin)
        .add_plugins(RenderOptionsPlugin)
        .add_plugins(LightingPlugin)
        .add_plugins(GameDebugUI)
        .add_plugins(StatisticsPlugin)
        .add_plugins(GameAudioPlugin)
        .add_plugins(CameraPlugin)
        .add_plugins(SkeletalAnimations);

    // Run
    app.run();
}
