mod gothic_asset_loader;
mod gothic_mesh;

use crate::gothic_mesh::create_gothic_world_mesh;

use crate::gothic_asset_loader::create_gothic_asset_loader;
use bevy::anti_alias::smaa::Smaa;
use bevy::{
    camera_controller::free_camera::{FreeCamera, FreeCameraPlugin, FreeCameraState},
    color::palettes::css::*,
    color::palettes::tailwind,
    prelude::*,
};
fn main() {
    App::new()
        .register_asset_source("gothic", create_gothic_asset_loader())
        .add_plugins(DefaultPlugins)
        // Plugin that enables FreeCamera functionality
        .add_plugins(FreeCameraPlugin)
        // Example code plugins
        .add_plugins((CameraPlugin, CameraSettingsPlugin, ScenePlugin))
        .run();
}

// Plugin that spawns the camera.
struct CameraPlugin;
impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_camera);
    }
}

fn spawn_camera(mut commands: Commands) {
    // ambient light
    // ambient lights' brightnesses are measured in candela per meter square, calculable as (color * brightness)
    commands.insert_resource(GlobalAmbientLight {
        color: WHITE.into(),
        brightness: 200.0,
        ..default()
    });
    commands.spawn((
        // AmbientLight {
        //     color: Color::linear_rgb(1.0, 1.0, 1.0),
        //     brightness: 1.0,
        //     affects_lightmapped_meshes: true,
        // },
        Smaa::default(),
        Camera3d::default(),
        Transform::from_xyz(0.0, 1.0, 0.0).looking_to(Vec3::X, Vec3::Y),
        // This component stores all camera settings and state, which is used by the FreeCameraPlugin to
        // control it. These properties can be changed at runtime, but beware the controller system is
        // constantly using and modifying those values unless the enabled field is false.
        FreeCamera {
            sensitivity: 0.2,
            friction: 25.0,
            walk_speed: 3.0,
            run_speed: 9.0,
            ..default()
        },
    ));
}

// Plugin that handles camera settings controls and information text
struct CameraSettingsPlugin;
impl Plugin for CameraSettingsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostStartup, spawn_text)
            // .add_systems(Update, update_cameras)
            .add_systems(Update, (update_camera_settings, update_text));
    }
}

#[derive(Component)]
struct InfoText;

fn spawn_text(mut commands: Commands, free_camera_query: Query<&FreeCamera>) {
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: px(-16),
            left: px(12),
            ..default()
        },
        children![Text::new(format!(
            "{}",
            free_camera_query.single().unwrap()
        ))],
    ));
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            bottom: px(12),
            left: px(12),
            ..default()
        },
        children![Text::new(concat![
            "Z/X: decrease/increase sensitivity\n",
            "C/V: decrease/increase friction\n",
            "F/G: decrease/increase scroll factor\n",
            "B: enable/disable controller",
        ]),],
    ));

    // Mutable text marked with component
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: px(12),
            right: px(12),
            ..default()
        },
        children![(InfoText, Text::new(""))],
    ));
}

// fn update_cameras(camera_query: Query<(Entity, &mut Camera3d)>) {
//     for (entity_id, _camera) in camera_query.iter() {
//         println!("Entity({:?})", entity_id);
//     }
//     // let (entity, mut camera) = camera_query.single_mut().unwrap();
// }
fn update_camera_settings(
    mut camera_query: Query<(&mut FreeCamera, &mut FreeCameraState)>,
    input: Res<ButtonInput<KeyCode>>,
) {
    let (mut free_camera, mut free_camera_state) = camera_query.single_mut().unwrap();

    if input.pressed(KeyCode::KeyZ) {
        free_camera.sensitivity = (free_camera.sensitivity - 0.005).max(0.005);
    }
    if input.pressed(KeyCode::KeyX) {
        free_camera.sensitivity += 0.005;
    }
    if input.pressed(KeyCode::KeyC) {
        free_camera.friction = (free_camera.friction - 0.2).max(0.0);
    }
    if input.pressed(KeyCode::KeyV) {
        free_camera.friction += 0.2;
    }
    if input.pressed(KeyCode::KeyF) {
        free_camera.scroll_factor = (free_camera.scroll_factor - 0.02).max(0.02);
    }
    if input.pressed(KeyCode::KeyG) {
        free_camera.scroll_factor += 0.02;
    }
    if input.just_pressed(KeyCode::KeyB) {
        free_camera_state.enabled = !free_camera_state.enabled;
    }
}

fn update_text(
    mut text_query: Query<&mut Text, With<InfoText>>,
    camera_query: Query<(&FreeCamera, &FreeCameraState)>,
) {
    let mut text = text_query.single_mut().unwrap();

    let (free_camera, free_camera_state) = camera_query.single().unwrap();

    text.0 = format!(
        "Enabled: {},\nSensitivity: {:.03}\nFriction: {:.01}\nScroll factor: {:.02}\nWalk Speed: {:.02}\nRun Speed: {:.02}\nSpeed: {:.02}",
        free_camera_state.enabled,
        free_camera.sensitivity,
        free_camera.friction,
        free_camera.scroll_factor,
        free_camera.walk_speed,
        free_camera.run_speed,
        free_camera_state.velocity.length(),
    );
}

// Plugin that spawns the scene and lighting.
struct ScenePlugin;
impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (spawn_lights, spawn_world));
    }
}

fn spawn_lights(mut commands: Commands) {
    // Main light
    commands.spawn((
        PointLight {
            color: Color::from(tailwind::ORANGE_300),
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(0.0, 3.0, 0.0),
    ));
    // Light behind wall
    commands.spawn((
        PointLight {
            color: Color::WHITE,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(-3.5, 3.0, 0.0),
    ));
    // Light under floor
    commands.spawn((
        PointLight {
            color: Color::from(tailwind::RED_300),
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(0.0, -0.5, 0.0),
    ));
}

fn spawn_world(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    asset_server: Res<AssetServer>,
) {
    // let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    // let floor = meshes.add(Plane3d::new(Vec3::Y, Vec2::splat(10.0)));
    // let sphere = meshes.add(Sphere::new(0.5));
    // let wall = meshes.add(Cuboid::new(0.2, 4.0, 3.0));

    // let blue_material = materials.add(Color::from(tailwind::BLUE_700));
    // let red_material = materials.add(Color::from(tailwind::RED_950));
    // let white_material = materials.add(Color::WHITE);

    let mesh = create_gothic_world_mesh();
    let mesh_handle = meshes.add(mesh);
    let mesh_material = materials.add(Color::WHITE);

    // NW_NATURE_BARK_04.TGA
    // let texture_handle = asset_server.load("FlightHelmet_Materials_LeatherPartsMat_BaseColor.png");
    let texture_handle = asset_server.load("gothic://earth.tga");

    let mesh_material = materials.add(StandardMaterial {
        base_color_texture: Some(texture_handle.clone()),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });

    commands.spawn((
        Mesh3d(mesh_handle.clone()),
        MeshMaterial3d(mesh_material.clone()),
    ));

    // let mesh = create_my_mesh();
    // let mesh_handle = meshes.add(mesh);
    // commands.spawn((
    //     Mesh3d(mesh_handle.clone()),
    //     MeshMaterial3d(red_material.clone()),
    // ));

    // Top side of floor
    // commands.spawn((
    //     Mesh3d(floor.clone()),
    //     MeshMaterial3d(white_material.clone()),
    // ));
    // // Under side of floor
    // commands.spawn((
    //     Mesh3d(floor.clone()),
    //     MeshMaterial3d(white_material.clone()),
    //     Transform::from_xyz(0.0, -0.01, 0.0).with_rotation(Quat::from_rotation_x(PI)),
    // ));
    // // Blue sphere
    // commands.spawn((
    //     Mesh3d(sphere.clone()),
    //     MeshMaterial3d(blue_material.clone()),
    //     Transform::from_xyz(3.0, 1.5, 0.0),
    // ));
    // // Tall wall
    // commands.spawn((
    //     Mesh3d(wall.clone()),
    //     MeshMaterial3d(white_material.clone()),
    //     Transform::from_xyz(-3.0, 2.0, 0.0),
    // ));
    // // Cube behind wall
    // commands.spawn((
    //     Mesh3d(cube.clone()),
    //     MeshMaterial3d(blue_material.clone()),
    //     Transform::from_xyz(-4.2, 0.5, 0.0),
    // ));
    // // Hidden cube under floor
    // commands.spawn((
    //     Mesh3d(cube.clone()),
    //     MeshMaterial3d(red_material.clone()),
    //     Transform {
    //         translation: Vec3::new(3.0, -2.0, 0.0),
    //         rotation: Quat::from_euler(EulerRot::YXZEx, FRAC_PI_4, FRAC_PI_4, 0.0),
    //         ..default()
    //     },
    // ));
    commands.spawn(
        // (
        SceneRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset("FlightHelmet.gltf"))),
        // Transform::from_xyz(0.0, 0.0, 2.0),
        // )
    );
}
