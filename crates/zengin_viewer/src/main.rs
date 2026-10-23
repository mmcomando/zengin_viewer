#![warn(missing_docs)]
//! Application to view world from Gothic 2 game

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
    clippy::too_many_arguments,
    clippy::wildcard_imports
)]

mod audio;
mod gameplay;
mod graphics;
mod gui;
mod gui_debug;
mod input;
mod xr;
mod zengin;

/// Directory of the main crate assets, the folder the
/// [`bevy::asset::AssetServer`] reads from. Resolved against the crate
/// directory so it is found regardless of the working directory.
pub fn main_assets_dir() -> &'static str {
    concat!(env!("CARGO_MANIFEST_DIR"), "/assets")
}

use crate::audio::SoundPlugin;
use crate::gameplay::camera::CameraPlugin;
use crate::gameplay::character::CharacterControllerPlugin;
use crate::gameplay::distance::DistancePlugin;
use crate::gameplay::game_time::GameTimePlugin;
use crate::graphics::{
    DiscoLamp, LightingPlugin, RenderOptionsPlugin, SkeletalAnimations, SkyBoxPlugin,
};
use crate::gui::GameLogPlugin;
use crate::gui_debug::GuiDebugPlugin;
use crate::input::{ActionsPlugin, Options};
use crate::xr::{XrPlugin, is_xr_enabled};
use crate::zengin::SpeechSounds;
use crate::zengin::ZenginPlugin;
use crate::zengin::{ZenginVfs, create_zengin_asset_loader, create_zengin_vfs};
use avian3d::prelude::*;

use bevy::prelude::*;
use bevy::window::PresentMode;

fn main() {
    let mut app = App::new();
    // Command line options, scenario handling has to be registered first.
    app.add_plugins(crate::input::OptionsPlugin);
    // This app custom assert source, has to be registered before default plugins.
    let vfs = create_zengin_vfs();
    app.insert_resource(ZenginVfs(vfs.clone()));
    app.insert_resource(SpeechSounds::new(vfs.clone()));
    app.register_asset_source("zengin", create_zengin_asset_loader(vfs));

    // Window options selection might be quite complicated if we consider XR/render-less/record/replay
    let window_plugin_options = window_plugin_options(&app);
    // XR or standard rendering
    if is_xr_enabled() {
        app.add_plugins(XrPlugin::new(window_plugin_options));
    } else if app.world().resource::<Options>().renderless() {
        crate::input::add_renderless_default_plugins(&mut app, window_plugin_options);
    } else {
        // Default plugin
        app.add_plugins(DefaultPlugins.set(window_plugin_options));
    }

    // Plugins from crates
    app.add_plugins(PhysicsPlugins::default());

    // This app custom plugins
    app.add_plugins(ActionsPlugin)
        // World and its content
        .add_plugins(ZenginPlugin)
        // Environment
        .add_plugins(SkyBoxPlugin)
        .add_plugins(LightingPlugin)
        .add_plugins(DiscoLamp)
        // Gameplay
        .add_plugins(CharacterControllerPlugin)
        .add_plugins(CameraPlugin)
        .add_plugins(DistancePlugin)
        .add_plugins(GameTimePlugin)
        .add_plugins(SkeletalAnimations)
        // Presentation and UI
        .add_plugins(RenderOptionsPlugin)
        .add_plugins(SoundPlugin)
        .add_plugins(GameLogPlugin)
        .add_plugins(crate::gui::HealthBarsPlugin)
        .add_plugins(crate::gui::InventoryUiPlugin);

    // The egui debug UI and gizmos are skipped for scenario replays so
    // screenshots contain no debug overlays, GuiDebugPlugin handles that.
    app.add_plugins(GuiDebugPlugin);

    // Run
    app.run();
}

fn window_plugin_options(app: &App) -> WindowPlugin {
    let mut primary_window = Window {
        title: "ZenGin Walker".into(),
        present_mode: PresentMode::AutoNoVsync,
        // present_mode: PresentMode::AutoVsync,
        ..default()
    };
    let replay_enabled = app.world().resource::<Options>().replay_actions().is_some();
    crate::input::apply_scenario_window_settings(&mut primary_window, replay_enabled);
    WindowPlugin {
        primary_window: Some(primary_window),
        ..default()
    }
}
