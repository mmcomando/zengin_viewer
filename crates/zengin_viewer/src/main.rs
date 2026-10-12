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
mod zengin_resources;

use crate::camera::CameraPlugin;
use crate::character::CharacterControllerPlugin;
use crate::game::animation::SkeletalAnimations;
use crate::game::audio::GameAudioPlugin;
use crate::game::dialog::GameDialogPlugin;
use crate::game::disco_lamp::DiscoLamp;
use crate::game::distance::DistancePlugin;
use crate::game::game_time::GameTimePlugin;
use crate::game::lighting::SunLight;
use crate::gui::camera::get_overlay_plugin;
use crate::gui::egui::GameDebugUI;
use crate::gui::statistics::StatisticsPlugin;
use crate::gui::toggle_visibility::ToggleVisibility;
use crate::gui::xr::{XrPlugin, is_xr_enabled};
use crate::input::InputPlugin;
use crate::skybox::SkyBoxPlugin;
use crate::zengin::ZenGinWorldPlugin;
use crate::zengin::loaders::vdf_reader::{create_zengin_asset_loader, create_zengin_vfs};
use crate::zengin::script::component::GothicScriptsPlugin;
use crate::zengin::sounds::SpeechSounds;
use avian3d::prelude::*;
use bevy::light::CascadeShadowConfigBuilder;

use bevy::window::PresentMode;
use bevy::{color::palettes::tailwind, prelude::*};

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
        .add_plugins(BasicContentPlugin)
        .add_plugins(ZenGinWorldPlugin)
        .add_plugins(SkyBoxPlugin)
        .add_plugins(ToggleVisibility)
        .add_plugins(CharacterControllerPlugin)
        .add_plugins(DiscoLamp)
        .add_plugins(DistancePlugin)
        .add_plugins(GameTimePlugin)
        .add_plugins(GameDebugUI)
        .add_plugins(StatisticsPlugin)
        .add_plugins(GothicScriptsPlugin)
        .add_plugins(GameDialogPlugin)
        .add_plugins(GameAudioPlugin)
        .add_plugins(CameraPlugin)
        .add_plugins(SkeletalAnimations);
    // Run
    app.run();
}

struct BasicContentPlugin;
impl Plugin for BasicContentPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_lights);
    }
}

// Lightning takes a LOT of resources as it is basicly rendering whole world with all entities
// TODO performance:
// - Draw only near objects
// - Limit objects casting shadows
// - Draw only near lights (frustum for lights takes considerable amount of time)
// - Animate only near objects
// - Less demanding shadows
const PREFER_PERF: bool = false;
// Shadow maps are very heavy and disco demo uses 4 of them tanking performance
// Enabling this make fps go from 300 to 80
const ENABLE_DISCO_SHADOW_MAPS: bool = false;

fn spawn_lights(mut commands: Commands) {
    let cascade_shadow_config = CascadeShadowConfigBuilder {
        // first_cascade_far_bound: 50.0, // default: 10.0
        // maximum_distance: 2000.0, // default: 150.0
        maximum_distance: 500.0, // default: 150.0
        ..default()
    }
    .build();

    // Initial values match noon of the day/night cycle from `game_time`.
    // The `update_day_night_lighting` system adjusts them every frame afterwards.
    let light = DirectionalLight {
        color: Color::from(tailwind::SKY_50),
        illuminance: bevy::light::light_consts::lux::OVERCAST_DAY,
        shadow_maps_enabled: !PREFER_PERF,
        // contact_shadows_enabled: true,
        ..default()
    };
    let ambient_light = GlobalAmbientLight {
        color: Color::linear_rgb(1.0, 1.0, 1.0),
        brightness: 200.0,
        ..default()
    };

    commands.insert_resource(ambient_light);

    commands.spawn((
        light,
        SunLight,
        Transform::from_xyz(1000.0, 1000.0, -10.0).looking_at(
            Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            Vec3::Y,
        ),
        cascade_shadow_config,
    ));
}
