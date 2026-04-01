use std::collections::HashMap;

use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};

use zen_kit_rs::{
    misc::{VfsOverwriteBehavior, VisualType},
    vfs::{Vfs, VfsNode},
    vobs::virtual_object::VirtualObject,
};

use crate::{gothic_unimplemented, warn_once};

fn get_world_pos(mut gothic_pos: Vec3) -> Vec3 {
    // X cords are mirrored
    gothic_pos.x = -gothic_pos.x;
    // World units are different
    gothic_pos / 100.0
}
fn get_world_rot(rot_mat: Mat3) -> Quat {
    let mut rot_euler = rot_mat.to_euler(EulerRot::XYZ);
    // Do to X cords beeing mirrored we also have to modify rotation
    rot_euler.1 = -rot_euler.1;
    let rot_quat = Quat::from_euler(EulerRot::XYZ, rot_euler.0, rot_euler.1, rot_euler.2);
    return rot_quat;
}

fn print_nodes(node: &VfsNode, level: u8) {
    let name = node.name();
    for _i in 0..level {
        print!(" ");
    }
    println!("{name}");

    let children = node.enumerate_children();
    for child in children {
        print_nodes(&child, level + 1);
    }
}

#[derive(Debug, Default)]
struct MeshData {
    indices: Vec<u32>,
    vertices: Vec<Vec3>,
    uvs: Vec<Vec2>,
    normals: Vec<Vec3>,
    colors: Vec<Vec4>,
}

#[derive(Debug)]
pub struct LoadedMeshData {
    pub texture: String,
    pub mesh: Mesh,
}

#[derive(Debug)]
pub struct MeshInstance {
    pub mesh_path: String,
    pub pos: Vec3,
    pub rot: Quat,
}

fn meshes_from_gothic_mrs_mesh(mesh: &zen_kit_rs::mrs_mesh::MrsMesh) -> Vec<LoadedMeshData> {
    let mut meshes: HashMap<String, MeshData> = HashMap::new();
    let sub_meshes = mesh.sub_meshes();
    let positions = mesh.positions();

    for sub_mesh in sub_meshes {
        let wedges = sub_mesh.wedges();
        let material = sub_mesh.material();
        let material_color = material.color();
        let texture_path = material.texture();

        if texture_path.is_empty() {
            continue;
        }

        let MeshData {
            uvs,
            normals,
            indices,
            vertices,
            colors,
        } = meshes.entry(texture_path.clone()).or_default();

        let material_color = Vec4::from_array([
            material_color.x as f32 / 255.0,
            material_color.y as f32 / 255.0,
            material_color.z as f32 / 255.0,
            material_color.w as f32 / 255.0,
        ]);

        let triangles = sub_mesh.triangles();

        for triangle in triangles {
            for index in triangle.wedges {
                let wedge = &wedges[index as usize];
                colors.push(material_color);
                indices.push(indices.len() as u32);

                vertices.push(get_world_pos(positions[wedge.index as usize]));
                uvs.push(wedge.texture);
                normals.push(wedge.normal);
            }
        }
    }

    let mut bevy_meshes: Vec<LoadedMeshData> = Vec::new();

    for (texture_str, mesh_data) in meshes {
        let mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, mesh_data.colors)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, mesh_data.vertices)
        .with_inserted_indices(Indices::U32(mesh_data.indices))
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, mesh_data.normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, mesh_data.uvs)
        .with_generated_tangents()
        .unwrap();
        bevy_meshes.push(LoadedMeshData {
            texture: texture_str,
            mesh,
        });
    }

    return bevy_meshes;
}

fn meshes_from_gothic_mesh(mesh: &zen_kit_rs::mesh::Mesh) -> Vec<LoadedMeshData> {
    let mut meshes: HashMap<String, MeshData> = HashMap::new();

    let polygons_count = mesh.polygon_count();
    // for polygon_index in 378936..378937 {
    for polygon_index in 0..polygons_count {
        let polygon = mesh.polygon(polygon_index);

        if polygon.is_portal() {
            // println!("Skip polygon({polygon_index}) it is portal");
            continue;
        }

        if polygon.is_occluder() {
            // println!("polygon({polygon_index}) is Occluder");
            // continue;
        }
        if polygon.is_sector() {
            // println!("polygon({polygon_index}) is Sector");
            // continue;
        }
        // if polygon.is_ouldRelight() {
        //     println!("polygon({polygon_index}) is ShouldRelight");
        // }
        if polygon.is_outdoor() {
            println!("polygon({polygon_index}) is Outdoor");
            continue;
        }
        if polygon.is_ghost_occluder() {
            panic!("polygon({polygon_index}) is GhostOccluder");
        }
        // if polygon.is_DynamicallyLit() {
        //     println!("polygon({polygon_index}) is DynamicallyLit");
        // }
        if polygon.is_lod() {
            panic!("polygon({polygon_index}) is Lod");
        }

        let polygon_indices = polygon.position_indices();
        let polygon_features_indices = polygon.feature_indices();

        assert!(polygon_features_indices.len() == polygon_indices.len());

        let material_index = polygon.material_index();
        let material = mesh.material(u64::from(material_index));
        let material_color = material.color();
        let texture_path = material.texture();

        if texture_path.is_empty() {
            // println!("Skip polygon({polygon_index}) it has empty texture");
            continue;
        }

        let MeshData {
            uvs,
            normals,
            indices,
            vertices,
            colors,
        } = meshes.entry(texture_path.clone()).or_default();

        let material_color = Vec4::from_array([
            material_color.x as f32 / 255.0,
            material_color.y as f32 / 255.0,
            material_color.z as f32 / 255.0,
            material_color.w as f32 / 255.0,
        ]);

        let triangles_num = polygon_indices.len() - 2;
        let trinagle_indices_num = 3;

        for triangle_index in 0..triangles_num {
            for index in 0..trinagle_indices_num {
                let idx = if index == 0 {
                    0
                } else {
                    triangle_index + index
                };
                let idx_feature = polygon_features_indices[idx];
                let feature = mesh.vertex(idx_feature as u64);
                uvs.push(feature.texture);
                normals.push(feature.normal);
            }
        }

        for triangle_index in 0..triangles_num {
            for index in 0..trinagle_indices_num {
                colors.push(material_color);
                indices.push(indices.len() as u32);

                let idx = if index == 0 {
                    0
                } else {
                    triangle_index + index
                };
                vertices.push(get_world_pos(mesh.position(polygon_indices[idx] as u64)));
            }
        }
    }

    let mut bevy_meshes: Vec<LoadedMeshData> = Vec::new();

    for (texture_str, mesh_data) in meshes {
        let mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, mesh_data.colors)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, mesh_data.vertices)
        .with_inserted_indices(Indices::U32(mesh_data.indices))
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, mesh_data.normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, mesh_data.uvs)
        .with_generated_tangents()
        .unwrap();
        bevy_meshes.push(LoadedMeshData {
            texture: texture_str,
            mesh,
        });
    }

    return bevy_meshes;
}

pub fn create_gothic_world_mesh() -> (HashMap<String, Vec<LoadedMeshData>>, Vec<MeshInstance>) {
    let vfs = Vfs::new();
    vfs.mount_disk_host(
        "/media/MM_HDD_DATA/SteamLibrary/steamapps/common/Gothic II/Data/Worlds.vdf",
        VfsOverwriteBehavior::ALL,
    );
    vfs.mount_disk_host(
        "/media/MM_HDD_DATA/SteamLibrary/steamapps/common/Gothic II/Data/Textures.vdf",
        VfsOverwriteBehavior::ALL,
    );
    vfs.mount_disk_host(
        "/media/MM_HDD_DATA/SteamLibrary/steamapps/common/Gothic II/Data/Meshes.vdf",
        VfsOverwriteBehavior::ALL,
    );
    vfs.mount_disk_host(
        "/media/MM_HDD_DATA/SteamLibrary/steamapps/common/Gothic II/Data/Meshes_Addon.vdf",
        VfsOverwriteBehavior::ALL,
    );
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
    let Some(node) = vfs.resolve_path(&mesh_path) else {
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
    return true;
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
                pos: pos,
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
    format!("/_WORK/DATA/MESHES/_COMPILED/{}", name)
}
