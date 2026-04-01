mod gui;
mod zengin;

use crate::gui::{CameraSettingsPlugin, get_overlay_plugin};
use crate::zengin::common::{LoadedMeshData, gothic2_dir};
use crate::zengin::loader_asset::create_gothic_asset_loader;
use crate::zengin::loader_texture::GothicTextureLoader;
use crate::zengin::script::*;
use crate::zengin::script_vm::ScriptVM;
use crate::zengin::world::create_gothic_world_mesh;
use avian3d::prelude::*;
use bevy::anti_alias::smaa::Smaa;
use bevy::core_pipeline::Skybox;
use bevy::light::CascadeShadowConfigBuilder;
use bevy::pbr::{ScreenSpaceAmbientOcclusion, ScreenSpaceAmbientOcclusionQualityLevel};
use bevy::platform::collections::HashMap;
use bevy::render::render_resource::{TextureViewDescriptor, TextureViewDimension};
use bevy::window::PresentMode;
use bevy::{
    camera_controller::free_camera::{FreeCamera, FreeCameraPlugin},
    color::palettes::tailwind,
    prelude::*,
};

fn main() {
    App::new()
        .register_asset_source("gothic", create_gothic_asset_loader())
        .add_plugins((DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "ZenGin Walker".into(),
                present_mode: PresentMode::Immediate,
                ..default()
            }),
            ..default()
        }),))
        .add_plugins(PhysicsPlugins::default())
        .init_asset_loader::<GothicTextureLoader>()
        // Plugin that enables FreeCamera functionality
        .add_plugins(FreeCameraPlugin)
        // Camera
        .add_plugins((CameraPlugin, CameraSettingsPlugin, ScenePlugin))
        .add_plugins(get_overlay_plugin())
        // Skybox update
        .add_systems(Update, skybox_update_texture)
        .run();
}

// Plugin that spawns the camera.
struct CameraPlugin;
impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_camera);
    }
}

fn spawn_camera(mut commands: Commands, asset_server: Res<AssetServer>) {
    let skybox_handle = asset_server.load("cubemap.png");

    commands.insert_resource(Cubemap {
        is_loaded: false,
        image_handle: skybox_handle.clone(),
    });

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
        Skybox {
            image: skybox_handle.clone(),
            brightness: 1000.0,
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
        maximum_distance: 2000.0,
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

#[derive(Resource)]
struct Cubemap {
    is_loaded: bool,
    image_handle: Handle<Image>,
}

fn skybox_update_texture(
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    mut cubemap: ResMut<Cubemap>,
    mut skyboxes: Query<&mut Skybox>,
) {
    if !cubemap.is_loaded && asset_server.load_state(&cubemap.image_handle).is_loaded() {
        let image = images.get_mut(&cubemap.image_handle).unwrap();
        // NOTE: PNGs do not have any metadata that could indicate they contain a cubemap texture,
        // so they appear as one texture. The following code reconfigures the texture as necessary.
        if image.texture_descriptor.array_layer_count() == 1 {
            image
                .reinterpret_stacked_2d_as_array(image.height() / image.width())
                .expect("asset should be 2d texture and height will always be evenly divisible with the given layers");
            image.texture_view_descriptor = Some(TextureViewDescriptor {
                dimension: Some(TextureViewDimension::Cube),
                ..default()
            });
        }

        for mut skybox in &mut skyboxes {
            skybox.image = cubemap.image_handle.clone();
        }

        cubemap.is_loaded = true;
    }
}

fn spawn_world(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    asset_server: Res<AssetServer>,
) {
    let path_str = gothic2_dir() + "/_work/Data/Scripts/_compiled/GOTHIC.DAT";
    let dat_data = parse_dat(&path_str).unwrap();
    let script_vm = ScriptVM::new(dat_data);
    let mut vm_state = crate::zengin::script_vm::State::new();
    script_vm.interpret_script_function(&mut vm_state, "startup_newworld");

    let mut world_data =
        create_gothic_world_mesh("/_WORK/DATA/WORLDS/NEWWORLD/NEWWORLD.ZEN", &vm_state);
    if false {
        let world_data_oldw =
            create_gothic_world_mesh("/_WORK/DATA/WORLDS/OLDWORLD/OLDWORLD.ZEN", &vm_state);
        for (mesh_path, data) in world_data_oldw.meshes {
            world_data.meshes.entry(mesh_path.clone()).or_insert(data);
        }
        for mut instance in world_data_oldw.mesh_instances {
            instance.pos += Vec3 {
                x: -150.0,
                y: 0.0,
                z: -900.0,
            };
            world_data.mesh_instances.push(instance);
        }
        world_data
            .light_instances
            .extend(world_data_oldw.light_instances);
    }
    println!("gothic_world_meshes len({})", world_data.meshes.len());

    let mut handles: HashMap<
        String,
        Vec<(
            Handle<Mesh>,
            Handle<StandardMaterial>,
            LoadedMeshData,
            StandardMaterial,
        )>,
    > = HashMap::new();

    for (model_path, mesh_data) in world_data.meshes {
        for data in mesh_data {
            let data_clone = data.clone();
            println!(
                "Add to draw model_path({model_path}), texture({})",
                data.texture
            );
            let texture = data.texture.replace(".TGA", "-C.TEX");
            let texture_full_path = format!("gothic://_WORK/DATA/TEXTURES/_COMPILED/{texture}");
            let mesh_handle = meshes.add(data.mesh);
            let texture_handle = asset_server.load(texture_full_path);

            let mut material = data.material;
            material.base_color_texture = Some(texture_handle.clone());

            let mesh_material = materials.add(material.clone());

            let arr = handles.entry(model_path.clone()).or_default();
            arr.push((mesh_handle, mesh_material, data_clone, material));
        }
    }

    println!(
        "Object intances number({})",
        world_data.mesh_instances.len()
    );
    for instance in world_data.mesh_instances {
        let Some(instance_data) = handles.get(&instance.mesh_path) else {
            println!("no data for mesh_path({})", &instance.mesh_path);
            continue;
        };
        for model_data in instance_data {
            let transform = Transform::from_translation(instance.pos).with_rotation(instance.rot);
            let transform = transform * model_data.2.transform;

            let mut material = model_data.1.clone();
            if let Some(override_texture) = &instance.texture_override {
                let texture = override_texture.replace(".TGA", "-C.TEX");
                let texture_full_path = format!("gothic://_WORK/DATA/TEXTURES/_COMPILED/{texture}");
                let mut mat = model_data.3.clone();
                let tex = asset_server.load(texture_full_path);
                mat.base_color_texture = Some(tex);
                material = materials.add(mat);
            }

            if instance.is_colider {
                commands.spawn((
                    RigidBody::Static,
                    ColliderConstructor::TrimeshFromMesh,
                    Mesh3d(model_data.0.clone()),
                    MeshMaterial3d(material),
                    transform,
                ));
            } else {
                commands.spawn((
                    Mesh3d(model_data.0.clone()),
                    MeshMaterial3d(material),
                    transform,
                ));
            }
        }
    }

    for instance in world_data.light_instances {
        let tr = Transform::from_translation(instance.pos).with_rotation(instance.rot);

        // let mesh_marker = handles
        //     .get("/_WORK/DATA/MESHES/_COMPILED/SPHERE.MRM")
        //     .unwrap();
        // let handle = &mesh_marker[0];
        // let tr_obj = tr.with_scale(Vec3::ONE * 0.1);
        // commands.spawn((
        //     Mesh3d(handle.0.clone()),
        //     MeshMaterial3d(handle.1.clone()),
        //     tr_obj,
        // ));

        commands.spawn((
            PointLight {
                color: Color::from(tailwind::ORANGE_300),
                intensity: light_consts::lumens::VERY_LARGE_CINEMA_LIGHT / 5.0,
                range: 5.0,
                ..default()
            },
            tr,
        ));
    }

    for x in -1..1 {
        for z in -1..1 {
            commands.spawn((
                RigidBody::Dynamic,
                Collider::cuboid(1.0, 1.0, 1.0),
                AngularVelocity(Vec3::new(2.5, 3.5, 1.5)),
                Mesh3d(meshes.add(Cuboid::from_length(1.0))),
                MeshMaterial3d(materials.add(Color::srgb_u8(124, 144, 255))),
                Transform::from_xyz(-30.0 + x as f32 * 5.0, 30.0, z as f32 * 5.0),
            ));
        }
    }
}
