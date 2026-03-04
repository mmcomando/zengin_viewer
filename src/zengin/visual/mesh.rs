use std::collections::HashMap;

use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};

use crate::zengin::{common::*, visual::material::get_standard_material};

pub fn meshes_from_gothic_mesh(mesh: &zen_kit_rs::mesh::Mesh) -> Vec<LoadedMeshData> {
    let mut meshes: HashMap<String, MeshData> = HashMap::new();

    let polygons_count = mesh.polygon_count();
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
        let zengin_material = mesh.material(u64::from(material_index));
        let material_color = zengin_material.color();
        let texture_path = zengin_material.texture();

        if texture_path.is_empty() {
            // println!("Skip polygon({polygon_index}) it has empty texture");
            continue;
        }

        let mesh_data = meshes.entry(texture_path.clone()).or_default();

        let material_color = Vec4::from_array([
            material_color.x as f32 / 255.0,
            material_color.y as f32 / 255.0,
            material_color.z as f32 / 255.0,
            material_color.w as f32 / 255.0,
        ]);

        mesh_data.material = get_standard_material(&zengin_material);

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
                mesh_data.uvs.push(feature.texture);
                mesh_data.normals.push(feature.normal);
            }
        }

        for triangle_index in 0..triangles_num {
            for index in 0..trinagle_indices_num {
                mesh_data.colors.push(material_color);
                mesh_data.indices.push(mesh_data.indices.len() as u32);

                let idx = if index == 0 {
                    0
                } else {
                    triangle_index + index
                };
                mesh_data
                    .vertices
                    .push(get_world_pos(mesh.position(polygon_indices[idx] as u64)));
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
            material: mesh_data.material,
            mesh,
            transform: Transform::IDENTITY,
            name: String::new(),
            head_transform: None,
        });
    }

    bevy_meshes
}
