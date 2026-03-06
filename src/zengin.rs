pub mod common;
pub mod loaders;
pub mod macros;
pub mod script;
pub mod visual;
pub mod world;

use crate::zengin::common::{ZenGinModel, gothic2_dir};
use crate::zengin::loaders::model::ZenGinModelLoader;
use crate::zengin::loaders::texture::ZenGinTextureLoader;
use crate::zengin::script::parse::*;
use crate::zengin::script::script_vm::ScriptVM;
use crate::zengin::visual::material::MatrialHashed;
use crate::zengin::world::load_gothic_world_data;
use avian3d::prelude::*;
use bevy::platform::collections::HashMap;
use bevy::{color::palettes::tailwind, prelude::*};

#[derive(Default)]
pub struct ZenGinWorldPlugin;

impl Plugin for ZenGinWorldPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<ZenGinModel>();
        app.init_asset_loader::<ZenGinTextureLoader>();
        app.init_asset_loader::<ZenGinModelLoader>();
        app.add_systems(Startup, insert_resources);
        app.add_systems(Startup, spawn_world.after(insert_resources));
        app.add_systems(Update, convert_zengin_model_to_entities);
    }
}

#[derive(Resource, Default)]
struct MaterialHandles {
    materials: HashMap<MatrialHashed, Handle<StandardMaterial>>,
    images: HashMap<String, Handle<Image>>,
    models: HashMap<String, Handle<ZenGinModel>>,
}

fn get_material_handle(
    materials: &mut ResMut<Assets<StandardMaterial>>,
    handles_map: &mut ResMut<MaterialHandles>,
    material: &StandardMaterial,
) -> Handle<StandardMaterial> {
    if let Some(handle) = handles_map.materials.get(&MatrialHashed(material.clone())) {
        return handle.clone();
    }
    let handle = materials.add(material.clone());
    handles_map
        .materials
        .insert(MatrialHashed(material.clone()), handle.clone());
    handle
}

fn get_image_handle(
    asset_server: &Res<AssetServer>,
    handles_map: &mut ResMut<MaterialHandles>,
    image_path: &str,
) -> Handle<Image> {
    if let Some(handle) = handles_map.images.get(image_path) {
        return handle.clone();
    }
    let handle = asset_server.load(image_path.to_string());
    handles_map
        .images
        .insert(image_path.to_string(), handle.clone());
    handle
}

fn get_model_handle(
    asset_server: &Res<AssetServer>,
    handles_map: &mut ResMut<MaterialHandles>,
    model_path: &str,
) -> Handle<ZenGinModel> {
    if let Some(handle) = handles_map.models.get(model_path) {
        return handle.clone();
    }
    let handle = asset_server.load(model_path.to_string());
    handles_map
        .models
        .insert(model_path.to_string(), handle.clone());
    handle
}

fn get_material_handle_full(
    asset_server: &Res<AssetServer>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    handles_map: &mut ResMut<MaterialHandles>,
    image_path: &str,
    mut material: StandardMaterial,
) -> Handle<StandardMaterial> {
    let tex_handle = get_image_handle(asset_server, handles_map, image_path);
    material.base_color_texture = Some(tex_handle.clone());
    get_material_handle(materials, handles_map, &material)
}

fn insert_resources(mut commands: Commands) {
    commands.insert_resource(MaterialHandles::default());
}

#[derive(Component, Default)]
struct ZenGinModelComponentLoaded {}
#[derive(Component)]
struct ZenGinModelComponent {
    model_handle: Handle<ZenGinModel>,
    override_texture: Option<String>,
}

#[allow(clippy::type_complexity)]
fn convert_zengin_model_to_entities(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    models: ResMut<Assets<ZenGinModel>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut material_handles: ResMut<MaterialHandles>,
    asset_server: Res<AssetServer>,
    query: Query<
        (Entity, &ZenGinModelComponent),
        (
            With<ZenGinModelComponent>,
            Without<ZenGinModelComponentLoaded>,
        ),
    >,
) {
    for (entity_id, model_component) in query.iter() {
        let handle = &model_component.model_handle;
        let Some(model) = models.get(handle) else {
            continue;
        };

        let mut entity = commands.entity(entity_id);
        entity.insert(ZenGinModelComponentLoaded::default());

        for sub_mesh in &model.sub_meshes {
            let texture = if let Some(texture) = &model_component.override_texture {
                texture
            } else {
                &sub_mesh.texture
            };
            let material_handle = get_material_handle_full(
                &asset_server,
                &mut materials,
                &mut material_handles,
                texture,
                sub_mesh.material.clone(),
            );

            let mesh_handle = meshes.add(sub_mesh.mesh.clone());

            entity.with_child((
                sub_mesh.transform,
                Mesh3d(mesh_handle),
                MeshMaterial3d(material_handle),
            ));
        }
    }
}

fn get_zen_gin_world_init_state() -> crate::zengin::script::script_vm::State {
    let path_str = gothic2_dir() + "/_work/Data/Scripts/_compiled/GOTHIC.DAT";
    let dat_data = parse_dat(&path_str).unwrap();
    let script_vm = ScriptVM::new(dat_data);
    let mut vm_state = crate::zengin::script::script_vm::State::new();
    script_vm.interpret_script_function(&mut vm_state, "startup_newworld");
    vm_state
}

fn spawn_world(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    asset_server: Res<AssetServer>,
    mut handles_map: ResMut<MaterialHandles>,
) {
    println!("\n-----SCRIPTS VM-----\n");
    let vm_state = get_zen_gin_world_init_state();
    println!("\n-----LOAD WORLD ZEN DATA-----\n");
    let mut world_data =
        load_gothic_world_data("/_WORK/DATA/WORLDS/NEWWORLD/NEWWORLD.ZEN", &vm_state);
    if false {
        let world_data_oldw =
            load_gothic_world_data("/_WORK/DATA/WORLDS/OLDWORLD/OLDWORLD.ZEN", &vm_state);
        world_data
            .light_instances
            .extend(world_data_oldw.light_instances);
    }

    println!("\n----CREATE ENGINE ENTITIES------\n");

    let world_model_handle: Handle<ZenGinModel> = asset_server.add(world_data.world_model);

    commands.spawn((
        Visibility::default(),
        ZenGinModelComponent {
            model_handle: world_model_handle,
            override_texture: None,
        },
        Transform::IDENTITY,
    ));

    for npc in world_data.npcs {
        commands.spawn((
            Visibility::default(),
            ZenGinModelComponent {
                model_handle: get_model_handle(&asset_server, &mut handles_map, &npc.body_model),
                override_texture: Some(npc.body_texture.clone()),
            },
            npc.body_tr,
        ));
        commands.spawn((
            Visibility::default(),
            ZenGinModelComponent {
                model_handle: get_model_handle(&asset_server, &mut handles_map, &npc.head_model),
                override_texture: Some(npc.head_texture.clone()),
            },
            npc.head_tr,
        ));
    }

    for instance in &world_data.static_models {
        let model_handle = get_model_handle(&asset_server, &mut handles_map, &instance.archetype);
        commands.spawn((
            ZenGinModelComponent {
                model_handle: model_handle.clone(),
                override_texture: None,
            },
            Visibility::default(),
            instance.tr,
        ));
    }

    for instance in world_data.light_instances {
        let tr = Transform::from_translation(instance.pos).with_rotation(instance.rot);
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
                #[allow(clippy::cast_precision_loss)]
                Transform::from_xyz(-30.0 + x as f32 * 5.0, 30.0, z as f32 * 5.0),
            ));
        }
    }
}
