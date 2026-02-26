use std::collections::HashMap;

use bevy::prelude::*;

use zen_kit_rs::{
    misc::{VfsOverwriteBehavior, VisualType},
    vfs::Vfs,
    vobs::virtual_object::VirtualObject,
};

use crate::{
    gothic_unimplemented,
    zengin::{common::*, mesh::meshes_from_gothic_mesh, mesh_mrs::meshes_from_gothic_mrs_mesh},
};

pub fn create_gothic_world_mesh() -> (HashMap<String, Vec<LoadedMeshData>>, Vec<MeshInstance>) {
    let vfs = Vfs::new();

    let vfs_override = VfsOverwriteBehavior::ALL;
    let dir = gothic2_dir();
    vfs.mount_disk_host(&format!("{}/Data/Worlds.vdf", dir), vfs_override);
    vfs.mount_disk_host(&format!("{}/Data/Textures.vdf", dir), vfs_override);
    vfs.mount_disk_host(&format!("{}/Data/Meshes.vdf", dir), vfs_override);
    vfs.mount_disk_host(&format!("{}/Data/Meshes_Addon.vdf", dir), vfs_override);

    let root_node = vfs.get_root();
    print_nodes(&root_node, 0);

    let world_path = "/_WORK/DATA/WORLDS/NEWWORLD/NEWWORLD.ZEN";
    let world_node = vfs.resolve_path(world_path).unwrap();
    let world_read = world_node.open().unwrap();
    let world = zen_kit_rs::world::World::load(&world_read).unwrap();

    let mut bevy_meshes = HashMap::new();
    let mut object_instances = Vec::new();

    let world_mesh = world.mesh();
    let world_bevy_meshes = meshes_from_gothic_mesh(&world_mesh);
    bevy_meshes.insert(world_path.to_string(), world_bevy_meshes);
    object_instances.push(MeshInstance {
        mesh_path: world_path.to_string(),
        pos: Vec3::ZERO,
        rot: Quat::IDENTITY,
    });

    for obj in world.root_objects() {
        load_meshes(&vfs, &mut bevy_meshes, &mut object_instances, &obj);
    }
    if !world.npcs().is_empty() {
        gothic_unimplemented!("Loading NPCs from world");
    }
    (bevy_meshes, object_instances)
}

fn load_mesh(
    mesh_path: &str,
    vfs: &Vfs,
    bevy_meshes: &mut HashMap<String, Vec<LoadedMeshData>>,
    mrs: bool,
) -> bool {
    if bevy_meshes.contains_key(mesh_path) {
        // println!("Already loaded mesh_path({mesh_path})");
        return true;
    }
    let Some(node) = vfs.resolve_path(mesh_path) else {
        return false;
    };
    let Some(read) = node.open() else {
        return false;
    };

    let meshes = if mrs {
        let mesh = zen_kit_rs::mrs_mesh::MrsMesh::load(&read).unwrap();
        meshes_from_gothic_mrs_mesh(&mesh)
    } else {
        let mesh = zen_kit_rs::mesh::Mesh::load(&read).unwrap();
        meshes_from_gothic_mesh(&mesh)
    };

    if meshes.is_empty() {
        gothic_unimplemented!("mesh_path({}) doesn't contain any meshes", mesh_path);
        return false;
    }
    // info!("Load mesh_path({})", mesh_path,);
    bevy_meshes.insert(mesh_path.to_string(), meshes);
    true
}

fn load_meshes(
    vfs: &Vfs,
    bevy_meshes: &mut HashMap<String, Vec<LoadedMeshData>>,
    object_instances: &mut Vec<MeshInstance>,
    obj: &VirtualObject,
) {
    let visual = obj.visual();
    let visual_name = visual.name();
    let visual_type = visual.get_type();
    let pos = get_world_pos(obj.position());
    let rot_quat = get_world_rot(obj.rotation());

    if !visual_name.is_empty() {
        let asset_path = match visual_type {
            VisualType::MULTI_RESOLUTION_MESH => {
                let asset_path = compiled_asset_path(&visual_name, ".3DS", ".MRM");
                if load_mesh(&asset_path, vfs, bevy_meshes, true) {
                    Some(asset_path)
                } else {
                    None
                }
            }
            VisualType::MESH => {
                let asset_path = compiled_asset_path(&visual_name, ".3DS", ".MSH");
                if load_mesh(&asset_path, vfs, bevy_meshes, false) {
                    Some(asset_path)
                } else {
                    None
                }
            }
            VisualType::DECAL => {
                warn_once!("GOTHIC unimplemented load: VisualType::DECAL");
                None
            }
            VisualType::PARTICLE_EFFECT => {
                warn_once!("GOTHIC unimplemented load: VisualType::PARTICLE_EFFECT");
                None
            }
            VisualType::CAMERA => {
                warn_once!("GOTHIC unimplemented load: VisualType::CAMERA");
                None
            }
            VisualType::MODEL => {
                warn_once!("GOTHIC unimplemented load: VisualType::MODEL");
                None
            }
            VisualType::MORPH_MESH => {
                warn_once!("GOTHIC unimplemented load: VisualType::MORPH_MESH, {}", 123);
                None
            }
            VisualType::UNKNOWN => {
                warn_once!("GOTHIC unimplemented load: VisualType::UNKNOWN");
                None
            }
        };

        if let Some(asset_path) = asset_path {
            object_instances.push(MeshInstance {
                mesh_path: asset_path,
                pos,
                rot: rot_quat,
            });
        }
    }
    for child in obj.children() {
        load_meshes(vfs, bevy_meshes, object_instances, &child);
    }
}

fn compiled_asset_path(present_name: &str, replace_from: &str, replace_to: &str) -> String {
    let name = present_name.replace(replace_from, replace_to);
    format!("/_WORK/DATA/MESHES/_COMPILED/{name}")
}
