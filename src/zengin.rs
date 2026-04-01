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
use crate::zengin::world::load_zengin_world_data;
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
        app.add_systems(
            Update,
            convert_zengin_model_to_entities.run_if(run_convert_zengin_model_to_entities),
        );
    }
}

#[derive(Resource, Default)]
struct MaterialHandles {
    materials: HashMap<MatrialHashed, Handle<StandardMaterial>>,
    images: HashMap<String, Handle<Image>>,
    models: HashMap<String, Handle<ZenGinModel>>,
}

impl MaterialHandles {
    fn get_material_handle(
        &mut self,
        materials: &mut ResMut<Assets<StandardMaterial>>,
        material: &StandardMaterial,
    ) -> Handle<StandardMaterial> {
        if let Some(handle) = self.materials.get(&MatrialHashed(material.clone())) {
            return handle.clone();
        }
        let handle = materials.add(material.clone());
        self.materials
            .insert(MatrialHashed(material.clone()), handle.clone());
        handle
    }

    fn get_image_handle(
        &mut self,
        asset_server: &Res<AssetServer>,
        image_path: &str,
    ) -> Handle<Image> {
        if let Some(handle) = self.images.get(image_path) {
            return handle.clone();
        }
        let handle = asset_server.load(image_path.to_string());
        self.images.insert(image_path.to_string(), handle.clone());
        handle
    }

    fn get_model_handle(
        &mut self,
        asset_server: &Res<AssetServer>,
        model_path: &str,
    ) -> Handle<ZenGinModel> {
        if let Some(handle) = self.models.get(model_path) {
            return handle.clone();
        }
        let handle = asset_server.load(model_path.to_string());
        self.models.insert(model_path.to_string(), handle.clone());
        handle
    }

    fn get_material_handle_full(
        &mut self,
        asset_server: &Res<AssetServer>,
        materials: &mut ResMut<Assets<StandardMaterial>>,
        image_path: &str,
        mut material: StandardMaterial,
    ) -> Handle<StandardMaterial> {
        let tex_handle = self.get_image_handle(asset_server, image_path);
        material.base_color_texture = Some(tex_handle.clone());
        self.get_material_handle(materials, &material)
    }
}

fn insert_resources(mut commands: Commands) {
    commands.insert_resource(MaterialHandles::default());
}

/// Adding this component will spawn child entities with 3d meshes contained in `model_handle`
#[derive(Component, Default)]
struct ZenGinModelComponent {
    model_handle: Handle<ZenGinModel>,
    override_texture: Option<String>,
    trimesh_collider: bool,
}

/// Check only entities which were not handled previously
#[derive(Component, Default)]
struct ZenGinModelComponentLoaded {}

fn run_convert_zengin_model_to_entities(
    query: Query<&ZenGinModelComponent, Without<ZenGinModelComponentLoaded>>,
) -> bool {
    query.iter().next().is_some()
}
#[allow(clippy::type_complexity)]
fn convert_zengin_model_to_entities(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    models: ResMut<Assets<ZenGinModel>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut material_handles: ResMut<MaterialHandles>,
    asset_server: Res<AssetServer>,
    query: Query<(Entity, &ZenGinModelComponent), (Without<ZenGinModelComponentLoaded>,)>,
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
            let material_handle = material_handles.get_material_handle_full(
                &asset_server,
                &mut materials,
                texture,
                sub_mesh.material.clone(),
            );

            let mesh_handle = meshes.add(sub_mesh.mesh.clone());
            if model_component.trimesh_collider {
                entity.with_child((
                    RigidBody::Static,
                    ColliderConstructor::TrimeshFromMesh,
                    sub_mesh.transform,
                    Mesh3d(mesh_handle),
                    MeshMaterial3d(material_handle),
                ));
            } else {
                entity.with_child((
                    sub_mesh.transform,
                    Mesh3d(mesh_handle),
                    MeshMaterial3d(material_handle),
                ));
            }
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
        load_zengin_world_data("/_WORK/DATA/WORLDS/NEWWORLD/NEWWORLD.ZEN", &vm_state);
    if false {
        let world_data_oldw =
            load_zengin_world_data("/_WORK/DATA/WORLDS/OLDWORLD/OLDWORLD.ZEN", &vm_state);
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
            trimesh_collider: true,
            ..default()
        },
        Transform::IDENTITY,
    ));

    for npc in world_data.npcs {
        commands.spawn((
            Visibility::default(),
            ZenGinModelComponent {
                model_handle: handles_map.get_model_handle(&asset_server, &npc.body_model),
                override_texture: Some(npc.body_texture.clone()),
                ..default()
            },
            npc.body_tr,
        ));
        commands.spawn((
            Visibility::default(),
            ZenGinModelComponent {
                model_handle: handles_map.get_model_handle(&asset_server, &npc.head_model),
                override_texture: Some(npc.head_texture.clone()),
                ..default()
            },
            npc.head_tr,
        ));
    }

    for instance in &world_data.static_models {
        let model_handle = handles_map.get_model_handle(&asset_server, &instance.archetype);
        commands.spawn((
            ZenGinModelComponent {
                model_handle: model_handle.clone(),
                ..default()
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
