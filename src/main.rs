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

mod game;
mod gui;
mod skybox;
mod toggle_visibility;
mod zengin;
mod zengin_resources;

use crate::gui::{CameraSettingsPlugin, get_overlay_plugin};
use crate::skybox::SkyBoxPlugin;
use crate::toggle_visibility::ToggleVisibility;
use crate::zengin::ZenGinWorldPlugin;
use crate::zengin::loaders::vdf_reader::create_zengin_asset_loader;
use avian3d::prelude::*;
use bevy::anti_alias::smaa::Smaa;
use bevy::light::CascadeShadowConfigBuilder;
use bevy::pbr::{ScreenSpaceAmbientOcclusion, ScreenSpaceAmbientOcclusionQualityLevel};
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
                present_mode: PresentMode::Immediate,
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
        Transform::from_xyz(40.0, 20.0, -10.0).looking_at(
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

fn spawn_lights(mut commands: Commands) {
    // Main light
    commands.spawn((
        PointLight {
            color: Color::from(tailwind::ORANGE_300),
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(-12.0, 3.0, 10.0),
    ));
    commands.spawn((
        PointLight {
            color: Color::from(tailwind::ORANGE_300),
            intensity: light_consts::lumens::VERY_LARGE_CINEMA_LIGHT,
            range: 100.0,
            radius: 10.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(-8.5, 1.0, -15.0),
    ));

    let cascade_shadow_config = CascadeShadowConfigBuilder {
        num_cascades: 4,
        first_cascade_far_bound: 50.0,
        maximum_distance: 2000.0,
        ..default()
    }
    .build();

    commands.insert_resource(GlobalAmbientLight {
        color: Color::linear_rgb(1.0, 1.0, 1.0),
        brightness: 100.0,
        ..default()
    });
    commands.spawn((
        DirectionalLight {
            color: Color::srgb_u8(172, 172, 193), // Moon color
            // color: Color::from(tailwind::SKY_50),
            // illuminance: bevy::light::light_consts::lux::FULL_MOON_NIGHT,
            illuminance: bevy::light::light_consts::lux::AMBIENT_DAYLIGHT / 15.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(4000.0, 2000.0, -10.0).looking_at(
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
