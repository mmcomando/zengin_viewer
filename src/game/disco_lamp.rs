use crate::PREFER_PERF;

use avian3d::math::PI;

use bevy::{color::palettes::tailwind, prelude::*};

#[derive(Default)]
pub struct DiscoLamp;

impl Plugin for DiscoLamp {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_disco);

        app.add_systems(Update, entity_rotate);
        app.add_systems(Update, draw_lights);
    }
}

#[derive(Component, Default)]
pub struct EntityRotate {}

#[derive(Component, Default)]
pub struct DrawGizmo {
    color: Color,
}

fn spawn_disco(mut commands: Commands) {
    let mut entity = commands.spawn((
        Visibility::default(),
        Transform::from_xyz(-99.0, 6.0, -8.0),
        EntityRotate::default(),
        DrawGizmo::default(),
    ));
    entity.with_child((
        PointLight {
            color: Color::from(tailwind::RED_500),
            intensity: light_consts::lumens::VERY_LARGE_CINEMA_LIGHT / 3.0,
            range: 100.0,
            radius: 10.0,
            shadows_enabled: !PREFER_PERF,
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 3.0),
        DrawGizmo {
            color: Color::from(tailwind::RED_500),
        },
    ));
    entity.with_child((
        PointLight {
            color: Color::from(tailwind::GREEN_500),
            intensity: light_consts::lumens::VERY_LARGE_CINEMA_LIGHT / 3.0,
            range: 100.0,
            radius: 10.0,
            shadows_enabled: !PREFER_PERF,
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, -3.0),
        DrawGizmo {
            color: Color::from(tailwind::GREEN_500),
        },
    ));
    entity.with_child((
        PointLight {
            color: Color::from(tailwind::BLUE_500),
            intensity: light_consts::lumens::VERY_LARGE_CINEMA_LIGHT / 3.0,
            range: 100.0,
            radius: 10.0,
            shadows_enabled: !PREFER_PERF,
            ..default()
        },
        Transform::from_xyz(3.0, 0.0, 0.0),
        DrawGizmo {
            color: Color::from(tailwind::BLUE_500),
        },
    ));
    entity.with_child((
        PointLight {
            color: Color::from(tailwind::PINK_500),
            intensity: light_consts::lumens::VERY_LARGE_CINEMA_LIGHT / 3.0,
            range: 100.0,
            radius: 10.0,
            shadows_enabled: !PREFER_PERF,
            ..default()
        },
        Transform::from_xyz(-3.0, 0.0, 0.0),
        DrawGizmo {
            color: Color::from(tailwind::PINK_500),
        },
    ));
}

fn entity_rotate(time: Res<Time>, query: Query<&mut Transform, With<EntityRotate>>) {
    let delta = time.delta_secs();
    for mut tr in query {
        tr.rotate_local_y(PI * delta * 0.05);
    }
}

fn draw_lights(mut gizmos: Gizmos, query: Query<(&GlobalTransform, &DrawGizmo)>) {
    for (tr, draw_gizmo) in query {
        gizmos.sphere(tr.translation(), 0.1, draw_gizmo.color);
    }
}
