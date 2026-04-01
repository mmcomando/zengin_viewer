use std::collections::HashMap;

use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology, VertexAttributeValues},
    prelude::*,
};

use crate::zengin::{common::*, visual::material::get_standard_material};

// fn is_forearm(
//     wedges_indices: &[u16],
//     wedges: &[MeshWedge],
//     soft_skin_mesh: Option<&zen_kit_rs::model::SoftSkinMesh>,
// ) -> bool {
//     for index in wedges_indices {
//         let wedge = &wedges[*index as usize];
//         let vertex_index = wedge.index as u64;

//         // mesh_data
//         //     .vertices
//         //     .push(get_world_pos(positions[vertex_index as usize]));

//         if let Some(skin) = soft_skin_mesh {
//             for weight_index in 0..skin.weight_count(vertex_index) {
//                 let weight_entry = skin.weight(vertex_index, weight_index);
//                 if weight_entry.weight > 0.7 && weight_entry.node_index == 14 {
//                     return true;
//                 }
//             }
//         }
//     }
//     return false;
// }

pub fn meshes_from_zengin_mrs_mesh(
    mesh: &zen_kit_rs::mrs_mesh::MrsMesh,
    soft_skin_mesh: Option<&zen_kit_rs::model::SoftSkinMesh>,
    final_tr: &[Transform],
) -> ZenGinModel {
    let mut meshes: HashMap<String, MeshData> = HashMap::new();
    let sub_meshes = mesh.sub_meshes();
    let positions = mesh.positions();

    let mut max_index: i32 = 0;
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
                let vertex_index = u64::from(wedge.index);

                if let Some(skin) = soft_skin_mesh {
                    let mut weights: [f32; 4] = [1.00, 0.00, 0.0, 0.0];
                    let mut indices: [u16; 4] = [0, 0, 0, 0];

                    // Orginal game uses final bone matrix and
                    //   position relative to bone to compute final vertex position
                    // We want to use bevy animation system which uses inverse bone matrix
                    //  and absolute model vertex position to compute final vertex position
                    // To achive this we compute avarage "absolute" model vertex position
                    //   from bone matrix and position relative to bone
                    let mut dt_avg = Vec3::ZERO;
                    for weight_index in 0..skin.weight_count(vertex_index) {
                        let weight_entry = skin.weight(vertex_index, weight_index);

                        let pos_local = get_world_pos(weight_entry.position);
                        let f_tr = final_tr[weight_entry.node_index as usize];
                        let dt = f_tr.to_matrix() * pos_local.to_homogeneous();
                        let dt = dt.truncate();

                        dt_avg += dt * weight_entry.weight;

                        let bone_index = weight_entry.node_index as usize;
                        weights[weight_index as usize] = weight_entry.weight;
                        indices[weight_index as usize] = bone_index as u16;
                        max_index = std::cmp::max(max_index, i32::from(weight_entry.node_index));
                    }

                    let pos = dt_avg;
                    mesh_data.vertices.push(pos);
                    mesh_data.weights.push(weights);
                    mesh_data.bone_indices.push(indices);
                } else {
                    mesh_data
                        .vertices
                        .push(get_world_pos(positions[vertex_index as usize]));
                }
                mesh_data.colors.push(material_color);
                mesh_data.indices.push(mesh_data.indices.len() as u32);
                mesh_data.uvs.push(wedge.texture);
                mesh_data.normals.push(wedge.normal);
            }
        }
    }

    let mut model = ZenGinModel::default();

    for (texture_str, mesh_data) in meshes {
        let has_skin = !mesh_data.bone_indices.is_empty();
        if has_skin {
            assert!(!mesh_data.bone_indices.is_empty());
            assert!(!mesh_data.weights.is_empty());
            assert!(mesh_data.weights.len() == mesh_data.vertices.len());
        }
        let mut mesh = Mesh::new(
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

        if has_skin {
            mesh.insert_attribute(
                Mesh::ATTRIBUTE_JOINT_INDEX,
                VertexAttributeValues::Uint16x4(mesh_data.bone_indices),
            );
            mesh.insert_attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT, mesh_data.weights);
            // TODO uncomment in bevy 0.19, this should improve frustum culling for animated objects
            // mesh.with_generated_skinned_mesh_bounds();
        }

        model.sub_meshes.push(ZenGinSubMesh {
            texture: get_full_texture_path(&texture_str),
            material: mesh_data.material,
            mesh,
            transform: Transform::IDENTITY,
            name: String::new(),
            collides: mesh_data.collides,
            is_skinned: has_skin,
        });
    }
    model
}
