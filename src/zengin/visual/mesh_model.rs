use bevy::prelude::*;
use zen_kit_rs::model::ModelHierarchyNode;

use crate::zengin::{common::*, visual::mesh_mrs::meshes_from_gothic_mrs_mesh};

pub fn meshes_from_gothic_model(model: &zen_kit_rs::model::Model) -> ZenGinModel {
    let model_mesh = model.mesh();
    return meshes_from_gothic_model_mesh(&model_mesh, Some(model));
}

pub fn meshes_from_gothic_model_mesh(
    model_mesh: &zen_kit_rs::model::ModelMesh,
    model_with_hierarchy: Option<&zen_kit_rs::model::Model>,
) -> ZenGinModel {
    let mut model = ZenGinModel::default();

    let soft_skin_meshes = model_mesh.meshes();
    let attachements = model_mesh.enumerate_attachments();

    for (name, mrs_mesh) in &attachements {
        let mut attachement_model = meshes_from_gothic_mrs_mesh(mrs_mesh);
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
        let mesh = soft_skin_mesh.mesh();
        let new_meshes = meshes_from_gothic_mrs_mesh(&mesh);
        model.sub_meshes.extend(new_meshes.sub_meshes);
    }

    if let Some(model_with_hierarchy) = model_with_hierarchy {
        let hierarchy = model_with_hierarchy.hierarchy();
        let nodes = hierarchy.nodes();

        // println!("GothicModel:");
        // if nodes.is_empty() {
        //     println!(" No Nodes")
        // } else {
        //     println!(" Nodes:");
        //     for node in &nodes {
        //         let tr = get_world_transform(node.transform);
        //         println!(
        //             "  name({}) parent_index({}) pos({}) rot({})",
        //             node.name, node.parent_index, tr.translation, tr.rotation
        //         );
        //     }
        // }

        let mut final_tr: Vec<Transform> = nodes
            .iter()
            .map(|node| get_world_transform(node.transform))
            .collect();

        for (i, node) in nodes.iter().enumerate() {
            if node.parent_index < 0 {
                compute_final_tr(i, &nodes, &mut final_tr);
            }
        }
        for i in 0..nodes.len() {
            if nodes[i].parent_index < 0 {
                compute_final_tr(i, &nodes, &mut final_tr);
            }
        }

        for new_mesh in &mut model.sub_meshes {
            // if let Some(node_index) = nodes.iter().position(|node| node.name == "BIP01 HEAD") {
            //     new_mesh.head_transform = Some(final_tr[node_index]);
            // }
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
        final_tr[node_index] =
            final_tr[node.parent_index as usize] * get_world_transform(node.transform);
    } else {
        final_tr[node_index] = get_world_transform(node.transform);
    }

    for (child_index, child) in nodes.iter().enumerate() {
        if child.parent_index == (node_index as i16) {
            compute_final_tr(child_index, nodes, final_tr);
        }
    }
}
