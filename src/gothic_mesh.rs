use std::collections::HashMap;

use bevy::{asset::RenderAssetUsages, mesh::Indices, mesh::PrimitiveTopology, prelude::*};

use zen_kit_rs::{
    misc::VfsOverwriteBehavior,
    vfs::{Vfs, VfsNode},
};

fn get_wolrld_pos(mut gothic_pos: Vec3) -> Vec3 {
    gothic_pos.x = -gothic_pos.x;
    gothic_pos / 100.0
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

pub fn create_gothic_world_mesh() -> HashMap<String, Mesh> {
    let mut meshes: HashMap<String, MeshData> = HashMap::new();

    let vfs = Vfs::new();
    vfs.mount_disk_host(
        "/media/MM_HDD_DATA/SteamLibrary/steamapps/common/Gothic II/Data/Worlds.vdf",
        VfsOverwriteBehavior::ALL,
    );
    vfs.mount_disk_host(
        "/media/MM_HDD_DATA/SteamLibrary/steamapps/common/Gothic II/Data/Textures.vdf",
        VfsOverwriteBehavior::ALL,
    );
    let root_node = vfs.get_root();
    print_nodes(&root_node, 0);

    let world_node = vfs
        .resolve_path("/_WORK/DATA/WORLDS/NEWWORLD/NEWWORLD.ZEN")
        .unwrap();
    println!("world_node {:?}", world_node);

    let world_read = world_node.open().unwrap();
    println!("world_read {:?}", world_read);
    let world = zen_kit_rs::world::World::load(&world_read).unwrap();
    println!("world {:?}", world);

    let mesh = world.mesh();
    println!("mesh {:?}", mesh);

    let positions_count = mesh.position_count();
    println!("Positions({positions_count}):");

    let polygons_count = mesh.polygon_count();
    println!("PolygonsCount({polygons_count}):");
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
                vertices.push(get_wolrld_pos(mesh.position(polygon_indices[idx] as u64)));
            }
        }
    }

    let mut bevy_meshes = HashMap::new();

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
        bevy_meshes.insert(texture_str, mesh);
    }

    bevy_meshes
}
