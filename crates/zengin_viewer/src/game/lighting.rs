use bevy::color::palettes::tailwind;
use bevy::core_pipeline::Skybox;
use bevy::light::CascadeShadowConfigBuilder;
use bevy::prelude::*;
use std::f32::consts::PI;

use super::game_time::GameTime;

/// Spawns the sun/moon light and adjusts day/night lighting from the game time.
pub struct LightingPlugin;

impl Plugin for LightingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_lights);
        app.add_systems(Update, update_day_night_lighting);
    }
}

/// Marks the directional light representing the sun/moon,
/// moved and adjusted by [`update_day_night_lighting`].
#[derive(Component, Default)]
pub struct SunLight;

const LIGHT_DISTANCE: f32 = 1000.0;
// Matches the original static light tilt (z was -10.0)
const LIGHT_TILT_Z: f32 = -10.0;
// Keeps the light above the horizon at sunrise, sunset and night
const MIN_LIGHT_ELEVATION: f32 = 0.3;
// Sun elevation over which the scene is fully in day lighting
const FULL_DAY_ELEVATION: f32 = 0.25;
// Day spans from 4:00 sunrise to 22:00 sunset, the remaining hours are night
const SUNRISE_HOUR: f32 = 4.0;
const SUNSET_HOUR: f32 = 22.0;

// Night light configuration, matches the previous static night setup
const NIGHT_ILLUMINANCE: f32 = bevy::light::light_consts::lux::FULL_MOON_NIGHT;
const NIGHT_LIGHT_COLOR: Color = Color::srgb_u8(172, 172, 193); // Moon color
const NIGHT_AMBIENT_BRIGHTNESS: f32 = 20.0;
const NIGHT_SKYBOX_BRIGHTNESS: f32 = 100.0;
// Day light configuration, matches the previous static day setup
const DAY_ILLUMINANCE: f32 = bevy::light::light_consts::lux::OVERCAST_DAY;
const DAY_SKYBOX_BRIGHTNESS: f32 = 1000.0;
const DAY_AMBIENT_BRIGHTNESS: f32 = 200.0;

// Lightning takes a LOT of resources as it is basicly rendering whole world with all entities
// TODO performance:
// - Draw only near objects
// - Limit objects casting shadows
// - Draw only near lights (frustum for lights takes considerable amount of time)
// - Animate only near objects
// - Less demanding shadows
const PREFER_PERF: bool = false;

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

/// Day from 4:00 sunrise to 22:00 sunset. At night the light acts as the moon.
pub(super) fn update_day_night_lighting(
    game_time: Res<GameTime>,
    mut sun_query: Query<(&mut DirectionalLight, &mut Transform), With<SunLight>>,
    mut ambient_light: ResMut<GlobalAmbientLight>,
    mut skyboxes: Query<&mut Skybox>,
) {
    let sun_angle = sun_elevation_angle(game_time.total_hours);
    let elevation = sun_angle.sin();
    let day_factor = day_light_factor(elevation);

    for (mut light, mut tr) in sun_query.iter_mut() {
        let light_elevation = elevation.abs().max(MIN_LIGHT_ELEVATION);
        tr.translation = Vec3::new(
            sun_angle.cos() * LIGHT_DISTANCE,
            light_elevation * LIGHT_DISTANCE,
            LIGHT_TILT_Z,
        );
        tr.look_at(Vec3::ZERO, Vec3::Y);

        light.color = NIGHT_LIGHT_COLOR.mix(&Color::from(tailwind::SKY_50), day_factor);
        light.illuminance = NIGHT_ILLUMINANCE + (DAY_ILLUMINANCE - NIGHT_ILLUMINANCE) * day_factor;
    }

    ambient_light.brightness =
        NIGHT_AMBIENT_BRIGHTNESS + (DAY_AMBIENT_BRIGHTNESS - NIGHT_AMBIENT_BRIGHTNESS) * day_factor;

    let skybox_brightness =
        NIGHT_SKYBOX_BRIGHTNESS + (DAY_SKYBOX_BRIGHTNESS - NIGHT_SKYBOX_BRIGHTNESS) * day_factor;
    for mut skybox in skyboxes.iter_mut() {
        skybox.brightness = skybox_brightness;
    }
}

/// Sun angle over the cycle: 0 at sunrise, PI/2 at zenith (13:00), PI at sunset.
/// During the night the angle continues from PI to 2*PI while the light acts as the moon.
fn sun_elevation_angle(total_hours: f32) -> f32 {
    let hour = if total_hours < SUNRISE_HOUR {
        total_hours + 24.0
    } else {
        total_hours
    };
    if hour <= SUNSET_HOUR {
        return (hour - SUNRISE_HOUR) / (SUNSET_HOUR - SUNRISE_HOUR) * PI;
    }
    let night_hours = 24.0 - (SUNSET_HOUR - SUNRISE_HOUR);
    return (hour - SUNSET_HOUR) / night_hours * PI + PI;
}

/// Smoothly transitions between night and day while the sun rises above the horizon.
fn day_light_factor(sun_elevation: f32) -> f32 {
    let t = (sun_elevation / FULL_DAY_ELEVATION).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
