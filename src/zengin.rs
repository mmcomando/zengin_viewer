pub mod common;
pub mod loaders;
pub mod macros;
pub mod script;
pub mod visual;
pub mod world;

use std::sync::Arc;

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

        app.add_systems(Update, toggle_visibility_world_mesh);
        app.add_systems(Update, toggle_visibility_static_meshes);
        app.add_systems(Update, toggle_visibility_npcs);

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
    commands.insert_resource(ToggleHide::default());
}

/// Adding this component will spawn child entities with 3d meshes contained in `model_handle`
#[derive(Component, Default)]
struct ZenGinModelComponent {
    model_handle: Handle<ZenGinModel>,
    override_texture: Option<String>,
    convex_colider: bool,
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

pub const STATIC_OBJECT: u32 = 1 << 1;
pub const DYNAMIC_OBJECT: u32 = 1 << 2;

#[allow(clippy::type_complexity)]
fn convert_zengin_model_to_entities(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    models: ResMut<Assets<ZenGinModel>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut material_handles: ResMut<MaterialHandles>,
    asset_server: Res<AssetServer>,
    query: Query<(Entity, &ZenGinModelComponent), Without<ZenGinModelComponentLoaded>>,
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
            if model_component.convex_colider {
                entity.with_child((
                    RigidBody::Static,
                    ColliderConstructor::ConvexHullFromMesh,
                    CollisionLayers::from_bits(STATIC_OBJECT, DYNAMIC_OBJECT),
                    sub_mesh.transform,
                    Mesh3d(mesh_handle),
                    MeshMaterial3d(material_handle),
                ));
            } else if model_component.trimesh_collider {
                entity.with_child((
                    RigidBody::Static,
                    ColliderConstructor::TrimeshFromMesh,
                    CollisionLayers::from_bits(STATIC_OBJECT, DYNAMIC_OBJECT),
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

#[derive(Component, Default)]
struct NpcVisibility {}

#[derive(Component, Default)]
struct WorldMesh {}

#[derive(Component, Default)]
struct StaticMesh {}

#[derive(Resource, Default)]
struct ToggleHide {
    show_world_mesh: bool,
    show_static_meshes: bool,
    show_npcs: bool,
}

fn toggle_visibility_world_mesh(
    keys: Res<ButtonInput<KeyCode>>,
    mut toggle_info: ResMut<ToggleHide>,
    mut query: Query<(&mut Visibility, &WorldMesh)>,
) {
    if !keys.just_pressed(KeyCode::KeyT) {
        return;
    }
    info!("Toggle world mesh visibility");
    toggle_info.show_world_mesh = !toggle_info.show_world_mesh;
    let vis = if toggle_info.show_world_mesh {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };
    for (mut visibility, _mesh) in &mut query {
        *visibility = vis;
    }
}

fn toggle_visibility_static_meshes(
    keys: Res<ButtonInput<KeyCode>>,
    mut toggle_info: ResMut<ToggleHide>,
    mut query: Query<(&mut Visibility, &StaticMesh)>,
) {
    if !keys.just_pressed(KeyCode::KeyV) {
        return;
    }
    info!("Toggle static meshes visibility");
    toggle_info.show_static_meshes = !toggle_info.show_static_meshes;
    let vis = if toggle_info.show_static_meshes {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };
    for (mut visibility, _mesh) in &mut query {
        *visibility = vis;
    }
}
fn toggle_visibility_npcs(
    keys: Res<ButtonInput<KeyCode>>,
    mut toggle_info: ResMut<ToggleHide>,
    mut query: Query<(&mut Visibility, &NpcVisibility)>,
) {
    if !keys.just_pressed(KeyCode::KeyB) {
        return;
    }
    info!("Toggle npcs visibility");
    toggle_info.show_npcs = !toggle_info.show_npcs;
    let vis = if toggle_info.show_npcs {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };
    for (mut visibility, _mesh) in &mut query {
        *visibility = vis;
    }
}

fn get_zen_gin_world_init_state() -> crate::zengin::script::script_vm::State {
    let _span = info_span!("InitScripts",).entered();
    let path_str = gothic2_dir() + "/_work/Data/Scripts/_compiled/GOTHIC.DAT";
    let dat_data = parse_dat(&path_str).unwrap();
    let dat_data = Arc::from(dat_data);
    let mut vm_state = crate::zengin::script::script_vm::State::new(dat_data.clone());
    let script_vm = ScriptVM::new(dat_data.clone());

    script_vm.initialize_variables(&mut vm_state);
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
    let _span = info_span!("spawn_world").entered();
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
        WorldMesh::default(),
    ));

    for npc in world_data.npcs {
        commands.spawn((
            Visibility::default(),
            ZenGinModelComponent {
                model_handle: handles_map.get_model_handle(&asset_server, &npc.body_model),
                override_texture: npc.body_texture.clone(),
                ..default()
            },
            npc.body_tr,
            NpcVisibility::default(),
        ));
        if let Some(head_model) = &npc.head_model {
            commands.spawn((
                Visibility::default(),
                ZenGinModelComponent {
                    model_handle: handles_map.get_model_handle(&asset_server, head_model),
                    override_texture: npc.head_texture.clone(),
                    ..default()
                },
                npc.head_tr,
                NpcVisibility::default(),
            ));
        }
        if let Some(armor_model) = &npc.armor_model {
            commands.spawn((
                Visibility::default(),
                ZenGinModelComponent {
                    model_handle: handles_map.get_model_handle(&asset_server, armor_model),
                    ..default()
                },
                npc.armor_tr,
                NpcVisibility::default(),
            ));
        }
    }
    for instance in &world_data.items {
        let model_handle = handles_map.get_model_handle(&asset_server, &instance.model);
        commands.spawn((
            ZenGinModelComponent {
                model_handle: model_handle.clone(),
                ..default()
            },
            Visibility::default(),
            instance.tr,
        ));
    }
    for instance in &world_data.static_models {
        let model_handle = handles_map.get_model_handle(&asset_server, &instance.archetype);
        commands.spawn((
            ZenGinModelComponent {
                model_handle: model_handle.clone(),
                convex_colider: true,
                ..default()
            },
            Visibility::default(),
            instance.tr,
            StaticMesh::default(),
        ));
    }

    for instance in world_data.light_instances {
        let tr = Transform::from_translation(instance.pos).with_rotation(instance.rot);
        commands.spawn((
            PointLight {
                color: Color::from(tailwind::ORANGE_300),
                intensity: light_consts::lumens::LUMENS_PER_HALOGEN_WATTS * 5000.0,
                range: 5.0,
                ..default()
            },
            tr,
        ));
    }

    for x in -4..4 {
        for z in -4..4 {
            commands.spawn((
                RigidBody::Dynamic,
                Collider::cuboid(1.0, 1.0, 1.0),
                AngularVelocity(Vec3::new(2.5, 3.5, 1.5)),
                Mesh3d(meshes.add(Cuboid::from_length(1.0))),
                MeshMaterial3d(materials.add(Color::srgb_u8(124, 144, 255))),
                CollisionLayers::from_bits(DYNAMIC_OBJECT, STATIC_OBJECT | DYNAMIC_OBJECT),
                #[allow(clippy::cast_precision_loss)]
                Transform::from_xyz(-30.0 + x as f32 * 5.0, 30.0, z as f32 * 5.0),
            ));
        }
    }
}
