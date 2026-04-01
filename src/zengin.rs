pub mod common;
pub mod loaders;
pub mod macros;
pub mod script;
pub mod visual;
pub mod world;

use crate::zengin::common::{LoadedMeshData, gothic2_dir};
use crate::zengin::script::script::*;
use crate::zengin::script::script_vm::ScriptVM;
use crate::zengin::world::create_gothic_world_mesh;
use avian3d::prelude::*;
use bevy::platform::collections::HashMap;
use bevy::{color::palettes::tailwind, prelude::*};

#[derive(Default)]
pub struct ZenGinWorldPlugin;

impl Plugin for ZenGinWorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_world);
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
    let mut vm_state = crate::zengin::script::script_vm::State::new();
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
    // println!("gothic_world_meshes len({})", world_data.meshes.len());

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
            // println!(
            //     "Add to draw model_path({model_path}), texture({})",
            //     data.texture
            // );
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

    // println!(
    //     "Object intances number({})",
    //     world_data.mesh_instances.len()
    // );
    for instance in world_data.mesh_instances {
        let Some(instance_data) = handles.get(&instance.mesh_path) else {
            println!("no data for mesh_path({})", &instance.mesh_path);
            panic!();
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
