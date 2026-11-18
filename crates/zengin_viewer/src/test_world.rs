//! Test worlds defined in code from cubes, spawned by the `TestWorld` scenario
//! command. They do not need a zengin world archive or the game scripts.

use std::collections::VecDeque;

use avian3d::prelude::*;
use bevy::dev_tools::infinite_grid::{InfiniteGrid, InfiniteGridPlugin, InfiniteGridSettings};
use bevy::prelude::*;

use crate::game_state::AppState;
use crate::gui_debug::WorldMesh;
use crate::zengin::{DYNAMIC_OBJECT, STATIC_OBJECT, ZenGinModelComponentLoaded};

/// Spawns the test worlds requested by scenario replays.
pub struct TestWorldPlugin;

impl Plugin for TestWorldPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PendingTestWorldSpawns>();
        app.init_resource::<TestWorldRoot>();
        app.add_plugins(InfiniteGridPlugin);
        app.add_systems(
            Update,
            spawn_test_world
                .run_if(in_state(AppState::LoadScreen).or_else(in_state(AppState::InGame))),
        );
    }
}

/// Test world spawn requests, drained by [`spawn_test_world`].
#[derive(Resource, Default)]
struct PendingTestWorldSpawns(VecDeque<String>);

/// Root entity of the most recently spawned test world.
#[derive(Resource, Default)]
struct TestWorldRoot(Option<Entity>);

pub(crate) fn request_test_world_spawn(world: &mut World, name: &str) {
    world
        .resource_mut::<PendingTestWorldSpawns>()
        .0
        .push_back(name.to_string());
}

/// Removes the test world geometry.
pub(crate) fn clear_test_world(world: &mut World) {
    let Some(entity) = world.resource_mut::<TestWorldRoot>().0.take() else {
        return;
    };
    // Another teardown (a world restart) may have removed it already.
    if let Err(error) = world.try_despawn(entity) {
        debug!("Test world root was already despawned: {error:?}");
    }
}

/// One cube of a test world, given by its center and full size in world units.
struct TestCube {
    center: Vec3,
    size: Vec3,
    color: Color,
}

const ROOM_HALF_SIZE: f32 = 120.0;
const ROOM_HEIGHT: f32 = 80.0;
const WALL_THICKNESS: f32 = 1.0;
const FLOOR_THICKNESS: f32 = 1.0;

/// Spacing of the floor debug grid lines.
const GRID_CELL_SIZE: f32 = 10.0;
/// Grid is lifted off the floor surface to avoid z-fighting with it.
const GRID_HEIGHT: f32 = 0.02;
const GRID_MINOR_LINE_COLOR: Color = Color::srgba(0.1, 0.1, 0.1, 0.35);
const GRID_MAJOR_LINE_COLOR: Color = Color::srgba(0.1, 0.1, 0.1, 0.6);
const GRID_X_AXIS_COLOR: Color = Color::srgb(0.9, 0.15, 0.15);
const GRID_Z_AXIS_COLOR: Color = Color::srgb(0.15, 0.3, 0.9);

/// Wall centers sit outside the room, so the `ROOM_HALF_SIZE` planes are the
/// inner wall faces.
const WALL_OFFSET: f32 = ROOM_HALF_SIZE + WALL_THICKNESS * 0.5;
const WALL_HEIGHT_MIDDLE: f32 = ROOM_HEIGHT * 0.5;

const SIMPLE_ROOM_CUBES: &[TestCube] = &[
    TestCube {
        center: Vec3::new(0.0, -FLOOR_THICKNESS * 0.5, 0.0),
        size: Vec3::new(WALL_OFFSET * 2.0, FLOOR_THICKNESS, WALL_OFFSET * 2.0),
        color: Color::srgb(1.0, 1.0, 1.0),
    },
    TestCube {
        center: Vec3::new(0.0, WALL_HEIGHT_MIDDLE, -WALL_OFFSET),
        size: Vec3::new(WALL_OFFSET * 2.0, ROOM_HEIGHT, WALL_THICKNESS),
        color: Color::srgb(0.95, 0.95, 0.95),
    },
    TestCube {
        center: Vec3::new(WALL_OFFSET, WALL_HEIGHT_MIDDLE, 0.0),
        size: Vec3::new(WALL_THICKNESS, ROOM_HEIGHT, WALL_OFFSET * 2.0),
        color: Color::srgb(0.93, 0.93, 0.93),
    },
    TestCube {
        center: Vec3::new(0.0, WALL_HEIGHT_MIDDLE, WALL_OFFSET),
        size: Vec3::new(WALL_OFFSET * 2.0, ROOM_HEIGHT, WALL_THICKNESS),
        color: Color::srgb(0.9, 0.9, 0.9),
    },
    TestCube {
        center: Vec3::new(-WALL_OFFSET, WALL_HEIGHT_MIDDLE, 0.0),
        size: Vec3::new(WALL_THICKNESS, ROOM_HEIGHT, WALL_OFFSET * 2.0),
        color: Color::srgb(0.88, 0.88, 0.88),
    },
    TestCube {
        center: Vec3::new(0.0, ROOM_HEIGHT + FLOOR_THICKNESS * 0.5, 0.0),
        size: Vec3::new(WALL_OFFSET * 2.0, FLOOR_THICKNESS, WALL_OFFSET * 2.0),
        color: Color::srgb(0.9, 0.95, 1.0),
    },
];

const TEST_WORLD_SIMPLE: &str = "simple";

fn spawn_test_world(
    mut pending: ResMut<PendingTestWorldSpawns>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut root: ResMut<TestWorldRoot>,
    mut cameras: Query<&mut Transform, With<Camera3d>>,
) {
    if pending.0.is_empty() {
        return;
    }
    for name in pending.0.drain(..) {
        let world_root = if name == TEST_WORLD_SIMPLE {
            create_simple_test_world(&mut commands, &mut meshes, &mut materials, &mut cameras)
        } else {
            panic!("Unknown test world({name})");
        };
        despawn_test_world(&mut commands, &mut root);
        root.0 = Some(world_root);
        info!("Spawned test world({name})");
    }
}

fn spawn_floor_grid(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            InfiniteGrid,
            InfiniteGridSettings {
                x_axis_color: GRID_X_AXIS_COLOR,
                z_axis_color: GRID_Z_AXIS_COLOR,
                minor_line_color: GRID_MINOR_LINE_COLOR,
                major_line_color: GRID_MAJOR_LINE_COLOR,
                // The shader scales plane coordinates, so the cell distance is 1/scale.
                scale: 1.0 / GRID_CELL_SIZE,
                // The room diagonal seen from the default camera is shorter than this.
                fadeout_distance: ROOM_HALF_SIZE * 2.0,
                ..default()
            },
            Transform::from_xyz(0.0, GRID_HEIGHT, 0.0),
        ))
        .id()
}

fn create_simple_test_world(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    cameras: &mut Query<&mut Transform, With<Camera3d>>,
) -> Entity {
    let mut children: Vec<Entity> = SIMPLE_ROOM_CUBES
        .iter()
        .map(|cube| spawn_cube(commands, meshes, materials, cube))
        .collect();
    children.push(spawn_floor_grid(commands));
    place_camera(
        cameras,
        TEST_WORLD_SIMPLE,
        Vec3::new(10.0, 10.0, 10.0),
        Vec3::ZERO,
    );
    spawn_world_root(commands, &children)
}

fn place_camera(
    cameras: &mut Query<&mut Transform, With<Camera3d>>,
    world_name: &str,
    position: Vec3,
    look_at: Vec3,
) {
    match cameras.single_mut() {
        Ok(mut camera) => {
            *camera = Transform::from_translation(position).looking_at(look_at, Vec3::Y);
        }
        Err(error) => warn!("Test world({world_name}): no single camera to place: {error:?}"),
    }
}

fn despawn_test_world(commands: &mut Commands, root: &mut TestWorldRoot) {
    let Some(entity) = root.0.take() else {
        return;
    };
    commands.entity(entity).try_despawn();
}

fn spawn_world_root(commands: &mut Commands, children: &[Entity]) -> Entity {
    let root = commands
        .spawn((
            Visibility::default(),
            Transform::IDENTITY,
            WorldMesh::default(),
            ZenGinModelComponentLoaded::default(),
        ))
        .id();
    commands.entity(root).add_children(children);
    root
}

fn spawn_cube(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    cube: &TestCube,
) -> Entity {
    let mesh = meshes.add(Cuboid::new(cube.size.x, cube.size.y, cube.size.z));
    let material = materials.add(StandardMaterial {
        base_color: cube.color,
        perceptual_roughness: 1.0,
        metallic: 0.0,
        ..default()
    });
    commands
        .spawn((
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::from_translation(cube.center),
            RigidBody::Static,
            Collider::cuboid(cube.size.x, cube.size.y, cube.size.z),
            CollisionLayers::from_bits(STATIC_OBJECT, DYNAMIC_OBJECT),
        ))
        .id()
}
