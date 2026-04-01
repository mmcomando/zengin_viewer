use bevy::prelude::*;
use zen_kit_rs::model::ModelHierarchyNode;

use crate::zengin::{common::*, visual::mesh_mrs::meshes_from_zengin_mrs_mesh};

pub fn meshes_from_zengin_model(model: &zen_kit_rs::model::Model) -> ZenGinModel {
    let model_mesh = model.mesh();
    return meshes_from_zengin_model_mesh(&model_mesh, Some(model));
}

pub fn meshes_from_zengin_model_mesh(
    model_mesh: &zen_kit_rs::model::ModelMesh,
    model_with_hierarchy: Option<&zen_kit_rs::model::Model>,
) -> ZenGinModel {
    let mut model = ZenGinModel::default();

    let soft_skin_meshes = model_mesh.meshes();
    let attachements = model_mesh.enumerate_attachments();

    for (name, mrs_mesh) in &attachements {
        let mut attachement_model = meshes_from_zengin_mrs_mesh(mrs_mesh);
        for sub_mesh in &mut attachement_model.sub_meshes {
            sub_mesh.name.clone_from(name);
        }
        model.sub_meshes.extend(attachement_model.sub_meshes);
    }

    // println!(
    //     "Loading ModelMesh soft_skin_meshes({}) attachements({}), with_hierarchy({})",
    //     soft_skin_meshes.len(),
    //     attachements.len(),
    //     model_with_hierarchy.is_some(),
    // );
    for soft_skin_mesh in &soft_skin_meshes {
        let new_model = meshes_from_zengin_soft_skin_mesh(soft_skin_mesh);
        model.sub_meshes.extend(new_model.sub_meshes);
    }

    if let Some(model_with_hierarchy) = model_with_hierarchy {
        let hierarchy = model_with_hierarchy.hierarchy();
        let nodes = hierarchy.nodes();

        let root_pos = get_world_pos(hierarchy.root_translation());

        // println!("ZenGinModel:");
        // if nodes.is_empty() {
        //     println!(" No Nodes")
        // } else {
        //     println!(" Nodes:");
        //     for node in &nodes {
        //         let tr = get_world_transform(node.transform);
        //         println!(
        //             "  name({:20}) parent_index({:3}) pos({:4.2}) rot({:.2})",
        //             node.name, node.parent_index, tr.translation, tr.rotation
        //         );
        //     }
        // }

        let mut final_tr: Vec<Transform> = nodes
            .iter()
            .map(|node| get_world_transform(node.transform))
            .collect();

        let root_tr = Transform::from_translation(root_pos);

        for i in 0..nodes.len() {
            if nodes[i].parent_index < 0 {
                final_tr[i] = root_tr * final_tr[i];
                compute_final_tr(i, &nodes, &mut final_tr);
            }
        }

        for (node_index, node) in nodes.iter().enumerate() {
            model
                .nodes_tr
                .insert(node.name.clone(), final_tr[node_index]);
        }
        for new_mesh in &mut model.sub_meshes {
            if let Some(node_index) = nodes.iter().position(|el| el.name == new_mesh.name) {
                new_mesh.transform = final_tr[node_index];
                // println!(
                //     "  set tr name({})  pos({}) rot({})",
                //     new_mesh.name, new_mesh.transform.translation, new_mesh.transform.rotation
                // );
            }
        }
    }
    model
}

fn compute_final_tr(node_index: usize, nodes: &[ModelHierarchyNode], final_tr: &mut [Transform]) {
    let node = &nodes[node_index];

    if node.parent_index >= 0 {
        final_tr[node_index] = final_tr[node.parent_index as usize] * final_tr[node_index];
    }

    for (child_index, child) in nodes.iter().enumerate() {
        if child.parent_index == (node_index as i16) {
            compute_final_tr(child_index, nodes, final_tr);
        }
    }
}

pub fn meshes_from_zengin_soft_skin_mesh(
    soft_skin_mesh: &zen_kit_rs::model::SoftSkinMesh,
) -> ZenGinModel {
    let mesh = soft_skin_mesh.mesh();
    let model = meshes_from_zengin_mrs_mesh(&mesh);

    // let vertices_count = mesh.position_count();
    // let mut skin_verter_data: Vec<[SoftSkinWeightEntry; 4]> =
    //     Vec::with_capacity(vertices_count as usize);
    // println!("soft_skin_mesh");
    // println!(" vertices_count({})", vertices_count);
    // println!(" node_count({})", soft_skin_mesh.node_count());
    // println!(" weight_total({})", soft_skin_mesh.weight_total());
    // for vertex_index in 0..vertices_count {
    //     let mut data: [SoftSkinWeightEntry; 4] = default();
    //     println!(
    //         "  vertex_index({vertex_index}) weight_count({})",
    //         soft_skin_mesh.weight_count(vertex_index)
    //     );
    //     for weight_index in 0..soft_skin_mesh.weight_count(vertex_index) {
    //         println!(
    //             "   weight({:?})",
    //             soft_skin_mesh.weight(vertex_index, weight_index)
    //         );
    //         data[weight_index as usize] = soft_skin_mesh.weight(vertex_index, weight_index);
    //     }

    //     skin_verter_data.push(data);
    // }

    // println!("skin data: {:#?}", skin_verter_data);

    return model;
}
