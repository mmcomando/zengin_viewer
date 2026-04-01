use std::collections::HashMap;

use bevy::prelude::*;
use zen_kit_rs::model::ModelHierarchyNode;

use crate::zengin::{common::*, visual::mesh_mrs::meshes_from_zengin_mrs_mesh};

pub fn meshes_from_zengin_model(
    model: &zen_kit_rs::model::Model,
    model_animation_hierarchy: Option<&zen_kit_rs::model::Model>,
) -> ZenGinModel {
    let model_mesh = model.mesh();
    return meshes_from_zengin_model_mesh(&model_mesh, model_animation_hierarchy, Some(model));
}

pub fn meshes_from_zengin_model_mesh(
    model_mesh: &zen_kit_rs::model::ModelMesh,
    model_animation_hierarchy: Option<&zen_kit_rs::model::Model>,
    model_own_hierarchy: Option<&zen_kit_rs::model::Model>,
) -> ZenGinModel {
    let mut model = ZenGinModel::default();

    let soft_skin_meshes = model_mesh.meshes();
    let attachements = model_mesh.enumerate_attachments();

    // println!(
    //     "Loading ModelMesh soft_skin_meshes({}) attachements({}), with_hierarchy({})",
    //     soft_skin_meshes.len(),
    //     attachements.len(),
    //     model_animation_hierarchy.is_some(),
    // );

    // For some models (mostly armor) we have two hierarchies
    // One which is used for skin weights (has only bones used by mesh)
    // Second one used by animations (main hierarchy, has all possible bones)
    // Even when we have two hierarchies all animations are backed for one "main" hierarchy
    // Due to this presence of two hierarchies we have to have mapping between
    //  "weight hierarchy" to "main hierarchy"
    // Final skinned mesh should use indices of bevy joint entities from "main hierarchy" but
    //   use inverse node positions from "weight hierarchy"
    let mut weight_to_animation_index: HashMap<usize, usize> = HashMap::new();
    let mut animation_bone_map: HashMap<String, usize> = HashMap::new();

    if model_own_hierarchy.is_some()
        && let Some(anim_model) = model_animation_hierarchy
    {
        let hierarchy = anim_model.hierarchy();
        let nodes = hierarchy.nodes();
        for (index, node) in nodes.iter().enumerate() {
            animation_bone_map.insert(node.name.clone(), index);
        }
    }
    let weights_hierarchy_model = model_own_hierarchy.or(model_animation_hierarchy);

    if let Some(weights_hierarchy_model) = weights_hierarchy_model {
        let hierarchy = weights_hierarchy_model.hierarchy();
        let nodes = hierarchy.nodes();

        let root_pos = get_world_pos(hierarchy.root_translation());
        let root_tr = Transform::from_translation(root_pos);

        // println!("ZenGinModel root_pos({:.2}):", root_pos);
        // if nodes.is_empty() {
        //     println!(" No Nodes");
        // } else {
        //     println!(" Nodes:");
        //     for (index, node) in nodes.iter().enumerate() {
        //         let tr = get_world_transform(node.transform);
        //         println!(
        //             "  node({index}) name({:20}) parent_index({:3}) pos({:4.2}) rot({:.2})",
        //             node.name, node.parent_index, tr.translation, tr.rotation
        //         );
        //     }
        // }

        let mut final_tr: Vec<Transform> = nodes
            .iter()
            .map(|node| get_world_transform(node.transform))
            .collect();

        model.nodes.clone_from(&final_tr);
        model.nodes.clone_from(&final_tr);
        model.parents = nodes.iter().map(|el| el.parent_index).collect();

        for i in 0..nodes.len() {
            weight_to_animation_index
                .insert(i, *animation_bone_map.get(&nodes[i].name).unwrap_or(&i));
            if nodes[i].parent_index < 0 {
                final_tr[i] = root_tr * final_tr[i];
                compute_final_tr(i, &nodes, &mut final_tr);
            }
        }
        model.inverse_bindposes = final_tr.iter().map(|el| el.to_matrix().inverse()).collect();
        model.final_tr.clone_from(&final_tr);
        model.node_names = nodes.iter().map(|el| el.name.clone()).collect();
        model.animation_bone_index = nodes
            .iter()
            .enumerate()
            .map(|(index, el)| *animation_bone_map.get(&el.name).unwrap_or(&index))
            .collect();
    }

    for (name, mrs_mesh) in &attachements {
        let mut attachement_model = meshes_from_zengin_mrs_mesh(mrs_mesh, None, &[]);
        for sub_mesh in &mut attachement_model.sub_meshes {
            sub_mesh.name.clone_from(name);
        }
        model.sub_meshes.extend(attachement_model.sub_meshes);
    }

    for soft_skin_mesh in &soft_skin_meshes {
        let mesh = soft_skin_mesh.mesh();
        // There are models which have weights but don't have bones hierarchy
        // Don't load weights for them as bevy doesn't like skinned meshes without bones data
        let soft_skin_mesh = if weights_hierarchy_model.is_some() {
            Some(soft_skin_mesh)
        } else {
            warn_once!("There are models with skin, but hierarchy is not present for them.");
            None
        };
        let new_model = meshes_from_zengin_mrs_mesh(&mesh, soft_skin_mesh, &model.final_tr);
        model.sub_meshes.extend(new_model.sub_meshes);
    }

    let is_skinned = model.sub_meshes.iter().any(|el| el.is_skinned);
    if is_skinned {
        assert!(weights_hierarchy_model.is_some());
    }

    for new_mesh in &mut model.sub_meshes {
        if let Some(node_index) = model.node_names.iter().position(|el| *el == new_mesh.name) {
            new_mesh.transform = model.final_tr[node_index];
            // println!(
            //     "  set tr name({})  pos({}) rot({})",
            //     new_mesh.name, new_mesh.transform.translation, new_mesh.transform.rotation
            // );
        }
    }

    // println!(
    //     "loaded model, submeshes({}), nodes({})",
    //     model.sub_meshes.len(),
    //     model.nodes.len()
    // );
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
