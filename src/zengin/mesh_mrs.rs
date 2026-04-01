use std::collections::HashMap;

use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};

use crate::zengin::common::*;

pub fn meshes_from_gothic_mrs_mesh(mesh: &zen_kit_rs::mrs_mesh::MrsMesh) -> Vec<LoadedMeshData> {
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

    bevy_meshes
}
