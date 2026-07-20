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

mod character;
mod game;
mod gui;
mod skybox;
mod zengin;
mod zengin_resources;

use crate::character::CharacterControllerPlugin;
use crate::game::animation::SkeletalAnimations;
use crate::game::disco_lamp::DiscoLamp;
use crate::game::distance::DistancePlugin;
use crate::gui::camera::{CameraSettingsPlugin, get_overlay_plugin};
use crate::gui::egui::GameDebugUI;
use crate::gui::statistics::StatisticsPlugin;
use crate::gui::toggle_visibility::ToggleVisibility;
use crate::skybox::SkyBoxPlugin;
use crate::zengin::ZenGinWorldPlugin;
use crate::zengin::loaders::vdf_reader::create_zengin_asset_loader;
use avian3d::prelude::*;
use bevy::anti_alias::smaa::Smaa;
use bevy::light::CascadeShadowConfigBuilder;
use bevy::pbr::{
    ContactShadows, ScreenSpaceAmbientOcclusion, ScreenSpaceAmbientOcclusionQualityLevel,
};
use bevy::window::PresentMode;
use bevy::{
    camera_controller::free_camera::{FreeCamera, FreeCameraPlugin},
    color::palettes::tailwind,
    prelude::*,
};

fn main() {
    App::new()
        // This app custom assert source, has to be regitred before default plugins
        .register_asset_source("zengin", create_zengin_asset_loader())
        // Default plugins
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "ZenGin Walker".into(),
                present_mode: PresentMode::AutoNoVsync,
                // present_mode: PresentMode::AutoVsync,
                ..default()
            }),
            ..default()
        }))
        // Plugins from crates
        .add_plugins(PhysicsPlugins::default())
        .add_plugins(FreeCameraPlugin)
        .add_plugins(get_overlay_plugin())
        // This app custom plugins
        .add_plugins(BasicContentPlugin)
        .add_plugins(ZenGinWorldPlugin)
        .add_plugins(CameraSettingsPlugin)
        .add_plugins(SkyBoxPlugin)
        .add_plugins(ToggleVisibility)
        .add_plugins(CharacterControllerPlugin)
        .add_plugins(DiscoLamp)
        .add_plugins(DistancePlugin)
        .add_plugins(GameDebugUI)
        .add_plugins(StatisticsPlugin)
        .add_plugins(SkeletalAnimations)
        // Run
        .run();
}

struct BasicContentPlugin;
impl Plugin for BasicContentPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_camera);
        app.add_systems(Startup, spawn_lights);
    }
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
const DAY: bool = true;

fn spawn_lights(mut commands: Commands) {
    let cascade_shadow_config = CascadeShadowConfigBuilder {
        // first_cascade_far_bound: 50.0, // default: 10.0
        // maximum_distance: 2000.0, // default: 150.0
        maximum_distance: 500.0, // default: 150.0
        ..default()
    }
    .build();

    let mut light = DirectionalLight {
        color: Color::srgb_u8(172, 172, 193), // Moon color
        illuminance: bevy::light::light_consts::lux::FULL_MOON_NIGHT,
        shadow_maps_enabled: !PREFER_PERF,
        // contact_shadows_enabled: true,
        ..default()
    };
    let mut ambient_light = GlobalAmbientLight {
        color: Color::linear_rgb(1.0, 1.0, 1.0),
        brightness: 20.0,
        ..default()
    };
    if DAY {
        ambient_light.brightness = 200.0;
        light.illuminance = bevy::light::light_consts::lux::OVERCAST_DAY;
        light.color = Color::from(tailwind::SKY_50);
    }

    commands.insert_resource(ambient_light);

    commands.spawn((
        light,
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
