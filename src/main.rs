mod gothic_asset_loader;
mod gothic_mesh;
mod gothic_texture_asset;
mod gui;
mod log_extend;

use bevy::anti_alias::smaa::Smaa;
use bevy::light::CascadeShadowConfigBuilder;
use bevy::platform::collections::HashMap;
use bevy::{
    camera_controller::free_camera::{FreeCamera, FreeCameraPlugin},
    color::palettes::tailwind,
    prelude::*,
};

use crate::gothic_asset_loader::create_gothic_asset_loader;
use crate::gothic_mesh::create_gothic_world_mesh;
use crate::gothic_texture_asset::GothicTextureLoader;
use crate::gui::CameraSettingsPlugin;

use avian3d::prelude::*;

fn main() {
    App::new()
        .register_asset_source("gothic", create_gothic_asset_loader())
        .add_plugins(DefaultPlugins)
        .add_plugins(PhysicsPlugins::default())
        .init_asset_loader::<GothicTextureLoader>()
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
    commands.spawn((
        Smaa::default(),
        Camera3d::default(),
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
            key_back: KeyCode::KeyR,
            key_right: KeyCode::KeyS,
            key_up: KeyCode::KeyF,
            ..default()
        },
    ));
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
        maximum_distance: 1000.0,
        ..default()
    }
    .build();

    commands.insert_resource(GlobalAmbientLight {
        color: Color::linear_rgb(1.0, 1.0, 1.0),
        brightness: 500.0,
        ..default()
    });
    commands.spawn((
        DirectionalLight {
            color: Color::srgb_u8(172, 172, 193), // Moon color
            illuminance: bevy::light::light_consts::lux::AMBIENT_DAYLIGHT / 2.0, // Full moon clear sky
            shadows_enabled: true,
            // affects_lightmapped_mesh_diffuse: bool,
            // shadow_depth_bias: f32,
            // shadow_normal_bias: f32,
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

fn spawn_world(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    asset_server: Res<AssetServer>,
) {
    let (gothic_world_meshes, object_instances) = create_gothic_world_mesh();
    println!("gothic_world_meshes len({})", gothic_world_meshes.len());

    let mut handles: HashMap<String, Vec<(Handle<Mesh>, Handle<StandardMaterial>)>> =
        HashMap::new();

    for (model_path, mesh_data) in gothic_world_meshes {
        for data in mesh_data {
            // println!(
            //     "Add to draw model_path({model_path}), texture({})",
            //     data.texture
            // );
            let texture = data.texture.replace(".TGA", "-C.TEX");
            let texture_full_path = format!("gothic://_WORK/DATA/TEXTURES/_COMPILED/{texture}");
            let mesh_handle = meshes.add(data.mesh);
            let texture_handle = asset_server.load(texture_full_path);

            let mesh_material = materials.add(StandardMaterial {
                base_color_texture: Some(texture_handle.clone()),
                alpha_mode: AlphaMode::Mask(0.5),
                cull_mode: None,
                double_sided: true,
                perceptual_roughness: 0.5,
                reflectance: 0.4,
                specular_tint: Color::BLACK,
                ..default()
            });

            let arr = handles.entry(model_path.clone()).or_default();
            arr.push((mesh_handle, mesh_material));
        }
    }

    for instance in object_instances {
        let Some(instance_data) = handles.get(&instance.mesh_path) else {
            println!("no data for mesh_path({})", &instance.mesh_path);
            continue;
        };
        for model_data in instance_data {
            commands.spawn((
                // RigidBody::Static,
                // ColliderConstructor::TrimeshFromMesh,
                Mesh3d(model_data.0.clone()),
                MeshMaterial3d(model_data.1.clone()),
                Transform::from_translation(instance.pos).with_rotation(instance.rot),
            ));
        }
    }

    // for x in -10..10 {
    //     for z in -10..10 {
    //         commands.spawn((
    //             RigidBody::Dynamic,
    //             Collider::cuboid(1.0, 1.0, 1.0),
    //             AngularVelocity(Vec3::new(2.5, 3.5, 1.5)),
    //             Mesh3d(meshes.add(Cuboid::from_length(1.0))),
    //             MeshMaterial3d(materials.add(Color::srgb_u8(124, 144, 255))),
    //             Transform::from_xyz(-30.0 + x as f32 * 5.0, 30.0, z as f32 * 5.0),
    //         ));
    //     }
    // }

    commands.spawn(SceneRoot(
        asset_server.load(GltfAssetLabel::Scene(0).from_asset("FlightHelmet.gltf")),
    ));
}
