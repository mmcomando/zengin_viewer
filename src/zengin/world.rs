use std::collections::HashMap;

use bevy::prelude::*;

use zen_kit_rs::{
    misc::{VfsOverwriteBehavior, VisualType},
    vfs::Vfs,
    vobs::virtual_object::VirtualObject,
};

use crate::{
    warn_unimplemented,
    zengin::{
        common::*,
        mesh::meshes_from_gothic_mesh,
        mesh_model::{meshes_from_gothic_model, meshes_from_gothic_model_mesh},
        mesh_mrs::meshes_from_gothic_mrs_mesh,
    },
};

const PLARCEHOLDER_MESH: &str = "/_WORK/DATA/MESHES/_COMPILED/SPHERE.MRM";

pub fn create_gothic_world_mesh(
    old_world: bool,
) -> (HashMap<String, Vec<LoadedMeshData>>, Vec<MeshInstance>) {
    let vfs = Vfs::new();

    let vfs_override = VfsOverwriteBehavior::ALL;
    let dir = gothic2_dir();
    vfs.mount_disk_host(&format!("{}/Data/Worlds.vdf", dir), vfs_override);
    vfs.mount_disk_host(&format!("{}/Data/Textures.vdf", dir), vfs_override);
    vfs.mount_disk_host(&format!("{}/Data/Meshes.vdf", dir), vfs_override);
    vfs.mount_disk_host(&format!("{}/Data/Meshes_Addon.vdf", dir), vfs_override);
    vfs.mount_disk_host(&format!("{}/Data/Anims.vdf", dir), vfs_override);
    vfs.mount_disk_host(&format!("{}/Data/Anims_Addon.vdf", dir), vfs_override);
    vfs.mount_disk_host(&format!("{}/Data/SystemPack.vdf", dir), vfs_override);

    if false {
        print_nodes(&vfs.get_root(), 0);
    }

    let world_path = if old_world {
        "/_WORK/DATA/WORLDS/OLDWORLD/OLDWORLD.ZEN"
    } else {
        "/_WORK/DATA/WORLDS/NEWWORLD/NEWWORLD.ZEN"
    };

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
        is_colider: true,
    });

    // Make sure that placeholder mesh is loaded
    load_mesh(PLARCEHOLDER_MESH, &vfs, &mut bevy_meshes);

    for obj in world.root_objects() {
        load_meshes(&vfs, &mut bevy_meshes, &mut object_instances, &obj);
    }
    if !world.npcs().is_empty() {
        warn_unimplemented!("Loading NPCs from world");
    }
    (bevy_meshes, object_instances)
}

fn load_mesh(
    mesh_path: &str,
    vfs: &Vfs,
    bevy_meshes: &mut HashMap<String, Vec<LoadedMeshData>>,
) -> bool {
    if bevy_meshes.contains_key(mesh_path) {
        // println!("Already loaded mesh_path({mesh_path})");
        return true;
    }
    let Some(node) = vfs.resolve_path(mesh_path) else {
        // warn!("Mesh({mesh_path}) not found");
        return false;
    };

    let Some(read) = node.open() else {
        warn!("Failed to open mesh({mesh_path})");
        return false;
    };

    let meshes = if mesh_path.ends_with(".MRM") {
        let mesh = zen_kit_rs::mrs_mesh::MrsMesh::load(&read).unwrap();
        meshes_from_gothic_mrs_mesh(&mesh)
    } else if mesh_path.ends_with(".MSH") {
        let mesh = zen_kit_rs::mesh::Mesh::load(&read).unwrap();
        meshes_from_gothic_mesh(&mesh)
    } else if mesh_path.ends_with(".MDL") {
        let mesh = zen_kit_rs::model::Model::load(&read).unwrap();
        meshes_from_gothic_model(&mesh)
    } else if mesh_path.ends_with(".MDM") {
        let mesh = zen_kit_rs::model::ModelMesh::load(&read).unwrap();
        // We try to load model only, but maybe there is coresponding hierarchy file
        // If we have hierarchy file load it and use it
        let hierarchy_path = mesh_path.replace("MDM", "MDH");
        let model_hierarchy = if let Some(hierarchy_node) = vfs.resolve_path(&hierarchy_path) {
            if let Some(read_hierarchy) = hierarchy_node.open() {
                zen_kit_rs::model::Model::load(&read_hierarchy)
            } else {
                None
            }
        } else {
            None
        };
        meshes_from_gothic_model_mesh(&mesh, model_hierarchy.as_ref())
    } else if mesh_path.ends_with(".MSB") {
        let mesh = zen_kit_rs::model::Model::load(&read).unwrap();
        meshes_from_gothic_model(&mesh)
    } else if mesh_path.ends_with(".MDH") {
        let mesh = zen_kit_rs::model::Model::load(&read).unwrap();
        meshes_from_gothic_model(&mesh)
    } else {
        println!("mesh_path({}) unrecognized mesh format", mesh_path);
        return false;
    };

    if meshes.is_empty() {
        warn_once!("mesh_path({}) doesn't contain any meshes", mesh_path);
        return false;
    }
    // info!("Load mesh_path({})", mesh_path,);
    bevy_meshes.insert(mesh_path.to_string(), meshes);
    true
}

fn try_load_mesh(
    asset_paths: &[String],
    vfs: &Vfs,
    bevy_meshes: &mut HashMap<String, Vec<LoadedMeshData>>,
) -> Option<String> {
    for asset_path in asset_paths {
        if load_mesh(asset_path, vfs, bevy_meshes) {
            return Some(asset_path.to_string());
        }
    }

    return None;
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
                if load_mesh(&asset_path, vfs, bevy_meshes) {
                    Some(asset_path)
                } else {
                    Some(PLARCEHOLDER_MESH.to_string())
                }
            }
            VisualType::MESH => {
                let asset_path = compiled_asset_path(&visual_name, ".3DS", ".MSH");
                if load_mesh(&asset_path, vfs, bevy_meshes) {
                    Some(asset_path)
                } else {
                    Some(PLARCEHOLDER_MESH.to_string())
                }
            }
            VisualType::DECAL => {
                warn_unimplemented!("load VisualType::DECAL");
                None
            }
            VisualType::PARTICLE_EFFECT => {
                warn_unimplemented!("load VisualType::PARTICLE_EFFECT");
                None
            }
            VisualType::CAMERA => {
                warn_unimplemented!("load VisualType::CAMERA");
                None
            }
            VisualType::MODEL => {
                let asset_paths: Vec<String> = vec![
                    // Model with hierarchy
                    format!(
                        "/_WORK/DATA/ANIMS/_COMPILED/{}",
                        visual_name.replace(".MDS", ".MDL")
                    ),
                    // Model only
                    format!(
                        "/_WORK/DATA/ANIMS/_COMPILED/{}",
                        visual_name.replace(".MDS", ".MDM")
                    ),
                ];

                if visual_name.ends_with(".ASC") {
                    warn_unimplemented!("load .ASC objects");
                    None
                } else if let Some(asset_path) = try_load_mesh(&asset_paths, vfs, bevy_meshes) {
                    warn!("load visual({})", visual_name);
                    // None
                    Some(asset_path)
                } else {
                    warn!("Failed to load visual({})", visual_name);
                    Some(PLARCEHOLDER_MESH.to_string())
                }
            }
            VisualType::MORPH_MESH => {
                warn_unimplemented!("load VisualType::MORPH_MESH");
                None
            }
            VisualType::UNKNOWN => {
                warn_unimplemented!("load VisualType::UNKNOWN");
                None
            }
        };

        if let Some(asset_path) = asset_path {
            object_instances.push(MeshInstance {
                mesh_path: asset_path,
                pos,
                rot: rot_quat,
                is_colider: false,
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
