pub mod common;
pub mod loaders;
pub mod macros;
pub mod script;
pub mod visual;
pub mod world;

use crate::zengin::common::{ZenGinSubMesh, get_full_texture_path, gothic2_dir};
use crate::zengin::script::parse::*;
use crate::zengin::script::script_vm::ScriptVM;
use crate::zengin::visual::material::MatrialHashed;
use crate::zengin::world::create_gothic_world_mesh;
use avian3d::prelude::*;
use bevy::platform::collections::HashMap;
use bevy::{color::palettes::tailwind, prelude::*};

#[derive(Default)]
pub struct ZenGinWorldPlugin;

impl Plugin for ZenGinWorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, insert_resources);
        app.add_systems(Startup, spawn_world.after(insert_resources));
    }
}

#[derive(Resource, Default)]
struct MaterialHandles {
    material_handles: HashMap<MatrialHashed, Handle<StandardMaterial>>,
    image_handles: HashMap<String, Handle<Image>>,
}

struct MeshData {
    mesh_handle: Handle<Mesh>,
    material_handle: Handle<StandardMaterial>,
    loaded_data: ZenGinSubMesh,
}

fn get_material_handle(
    materials: &mut ResMut<Assets<StandardMaterial>>,
    handles_map: &mut ResMut<MaterialHandles>,
    material: &StandardMaterial,
) -> Handle<StandardMaterial> {
    if let Some(handle) = handles_map
        .material_handles
        .get(&MatrialHashed(material.clone()))
    {
        return handle.clone();
    }
    let handle = materials.add(material.clone());
    handles_map
        .material_handles
        .insert(MatrialHashed(material.clone()), handle.clone());
    handle
}

fn get_image_handle(
    asset_server: &Res<AssetServer>,
    handles_map: &mut ResMut<MaterialHandles>,
    image_path: &str,
) -> Handle<Image> {
    if let Some(handle) = handles_map.image_handles.get(image_path) {
        return handle.clone();
    }
    let handle = asset_server.load(image_path.to_string());
    handles_map
        .image_handles
        .insert(image_path.to_string(), handle.clone());
    handle
}

fn get_material_handle_full(
    asset_server: &Res<AssetServer>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    handles_map: &mut ResMut<MaterialHandles>,
    image_path: &str,
    mut material: StandardMaterial,
) -> Handle<StandardMaterial> {
    let tex_handle = get_image_handle(&asset_server, handles_map, image_path);
    material.base_color_texture = Some(tex_handle.clone());
    get_material_handle(materials, handles_map, &material)
}

fn insert_resources(mut commands: Commands) {
    commands.insert_resource(MaterialHandles::default());
}

fn spawn_world(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut material_handles: ResMut<MaterialHandles>,
    asset_server: Res<AssetServer>,
) {
    let path_str = gothic2_dir() + "/_work/Data/Scripts/_compiled/GOTHIC.DAT";
    let dat_data = parse_dat(&path_str).unwrap();
    let script_vm = ScriptVM::new(dat_data);
    let mut vm_state = crate::zengin::script::script_vm::State::new();
    script_vm.interpret_script_function(&mut vm_state, "startup_newworld");

    let mut world_data =
        create_gothic_world_mesh("/_WORK/DATA/WORLDS/NEWWORLD/NEWWORLD.ZEN", &vm_state);
    if false {
        let world_data_oldw =
            create_gothic_world_mesh("/_WORK/DATA/WORLDS/OLDWORLD/OLDWORLD.ZEN", &vm_state);
        for (mesh_path, data) in world_data_oldw.model_archetypes {
            world_data
                .model_archetypes
                .entry(mesh_path.clone())
                .or_insert(data);
        }
        // for mut instance in world_data_oldw.static_models {
        //     instance.tr.translation += Vec3 {
        //         x: -150.0,
        //         y: 0.0,
        //         z: -900.0,
        //     };
        //     world_data.static_models.push(instance);
        // }
        world_data
            .light_instances
            .extend(world_data_oldw.light_instances);
    }
    // println!("gothic_world_meshes len({})", world_data.meshes.len());

    let mut handles: HashMap<String, Vec<MeshData>> = HashMap::new();

    let mut created_materials = 0;
    for (model_path, model) in world_data.model_archetypes {
        for data in model.sub_meshes {
            let data_clone = data.clone();
            // println!(
            //     "Add to draw model_path({model_path}), texture({})",
            //     data.texture
            // );
            let mesh_handle = meshes.add(data.mesh);
            let texture_handle =
                get_image_handle(&asset_server, &mut material_handles, &data.texture);

            let mut material = data.material;
            material.base_color_texture = Some(texture_handle.clone());

            let mesh_material =
                get_material_handle(&mut materials, &mut material_handles, &material);
            created_materials += 1;

            let arr = handles.entry(model_path.clone()).or_default();
            arr.push(MeshData {
                mesh_handle,
                material_handle: mesh_material,
                loaded_data: data_clone,
            });
        }
    }

    // println!(
    //     "Object intances number({})",
    //     world_data.mesh_instances.len()
    // );

    for sub_mesh in world_data.world_meshes {
        let material_handle = get_material_handle_full(
            &asset_server,
            &mut materials,
            &mut material_handles,
            &sub_mesh.texture,
            sub_mesh.material,
        );

        let mesh_handle = meshes.add(sub_mesh.mesh);
        let transform = Transform::IDENTITY;
        commands.spawn((
            RigidBody::Static,
            ColliderConstructor::TrimeshFromMesh,
            Mesh3d(mesh_handle),
            MeshMaterial3d(material_handle),
            transform,
        ));
    }

    for npc in world_data.npcs {
        {
            let sub_mesh = &npc.body_model.sub_meshes[0];

            let material_handle = get_material_handle_full(
                &asset_server,
                &mut materials,
                &mut material_handles,
                &npc.body_texture,
                sub_mesh.material.clone(),
            );

            let mesh_handle = meshes.add(sub_mesh.mesh.clone());
            let transform = npc.body_tr;
            commands.spawn((
                RigidBody::Static,
                ColliderConstructor::TrimeshFromMesh,
                Mesh3d(mesh_handle),
                MeshMaterial3d(material_handle),
                transform,
            ));
        }
        {
            let sub_mesh = &npc.head_model.sub_meshes[0];

            let material_handle = get_material_handle_full(
                &asset_server,
                &mut materials,
                &mut material_handles,
                &npc.head_texture,
                sub_mesh.material.clone(),
            );

            let mesh_handle = meshes.add(sub_mesh.mesh.clone());
            let transform = npc.head_tr;
            commands.spawn((
                RigidBody::Static,
                ColliderConstructor::TrimeshFromMesh,
                Mesh3d(mesh_handle),
                MeshMaterial3d(material_handle),
                transform,
            ));
        }
    }

    for instance in world_data.static_models {
        let Some(instance_data) = handles.get(&instance.archetype) else {
            println!("no data for mesh_path({})", &instance.archetype);
            continue;
        };
        for model_data in instance_data {
            let transform = instance.tr * model_data.loaded_data.transform;

            let material = model_data.material_handle.clone();
            commands.spawn((
                Mesh3d(model_data.mesh_handle.clone()),
                MeshMaterial3d(material),
                transform,
            ));
        }
    }

    println!(
        "created_materials({created_materials}), uniq_images({}), uniq_materials({})",
        material_handles.image_handles.len(),
        material_handles.material_handles.len(),
    );

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
                #[allow(clippy::cast_precision_loss)]
                Transform::from_xyz(-30.0 + x as f32 * 5.0, 30.0, z as f32 * 5.0),
            ));
        }
    }
}
