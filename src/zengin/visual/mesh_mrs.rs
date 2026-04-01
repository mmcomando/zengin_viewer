use std::collections::HashMap;

use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};

use crate::zengin::{common::*, visual::material::get_standard_material};

pub fn meshes_from_zengin_mrs_mesh(mesh: &zen_kit_rs::mrs_mesh::MrsMesh) -> ZenGinModel {
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

        let mesh_data = meshes.entry(texture_path.clone()).or_default();

        mesh_data.material = get_standard_material(&material);
        mesh_data.collides = !material.disable_collision();

        let material_color = Vec4::from_array([
            f32::from(material_color.x) / 255.0,
            f32::from(material_color.y) / 255.0,
            f32::from(material_color.z) / 255.0,
            f32::from(material_color.w) / 255.0,
        ]);

        let triangles = sub_mesh.triangles();

        for triangle in triangles {
            for index in triangle.wedges {
                let wedge = &wedges[index as usize];
                mesh_data.colors.push(material_color);
                mesh_data.indices.push(mesh_data.indices.len() as u32);

                mesh_data
                    .vertices
                    .push(get_world_pos(positions[wedge.index as usize]));
                mesh_data.uvs.push(wedge.texture);
                mesh_data.normals.push(wedge.normal);
            }
        }
    }

    let mut model = ZenGinModel::default();
    for (texture_str, mesh_data) in meshes {
        let mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        )
        // .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, mesh_data.colors)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, mesh_data.vertices)
        .with_inserted_indices(Indices::U32(mesh_data.indices))
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, mesh_data.normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, mesh_data.uvs)
        .with_generated_tangents()
        .unwrap();
        model.sub_meshes.push(ZenGinSubMesh {
            texture: get_full_texture_path(&texture_str),
            material: mesh_data.material,
            mesh,
            transform: Transform::IDENTITY,
            name: String::new(),
            collides: mesh_data.collides,
        });
    }
    model
}
