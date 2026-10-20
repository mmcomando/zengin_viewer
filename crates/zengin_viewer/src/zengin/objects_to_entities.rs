use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32};

use crate::gameplay::distance::{ClassifyDistance, DistanceFar, DistanceNear};
use crate::gameplay::objects::NpcModel;
use crate::graphics::animation::{
    AnimatedJoint, AnimationController, AnimationData, AnimationState, BoneAnimationData,
    FrameState, NpcAnimations,
};
use crate::gui_debug::toggle_visibility::NpcVisibility;
use crate::warn_once;
use crate::zengin::GameNpc;
use crate::zengin::common::{ZenGinModel, get_animation_path};
use crate::zengin::npc_routines::NpcWeaponSlots;
use crate::zengin::zengin_resources::{
    MaterialHandles, ZenGinModelComponent, despawn_visual_when_far_resource,
};
use bevy::mesh::skinning::{SkinnedMesh, SkinnedMeshInverseBindposes};
use bevy::prelude::*;

#[derive(Default)]
pub struct GameObjectSpawnEntities;

impl Plugin for GameObjectSpawnEntities {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, spawn_npcs);

        app.add_systems(Update, add_classify_distance);
        app.add_systems(
            Last,
            despawn_visual_when_far.after(despawn_visual_when_far_resource),
        );
    }
}

// const EXAMPLE_ANIMATION: &str = "zengin://_WORK/DATA/ANIMS/_COMPILED/HUMANS-T_YES.MAN";
// const EXAMPLE_ANIMATION: &str = "zengin://_WORK/DATA/ANIMS/_COMPILED/HUMANS-S_WALKL.MAN";
// const EXAMPLE_ANIMATION: &str = "zengin://_WORK/DATA/ANIMS/_COMPILED/HUMANS-S_WALKWL.MAN";
// const EXAMPLE_ANIMATION: &str = "zengin://_WORK/DATA/ANIMS/_COMPILED/HUMANS-S_WASH.MAN";
const EXAMPLE_ANIMATION: &str = "zengin://_WORK/DATA/ANIMS/_COMPILED/HUMANS-S_FIRE_VICTIM.MAN";

// const EXAMPLE_ANIMATION: &str = "zengin://_WORK/DATA/ANIMS/_COMPILED/HUMANS-S_WATCHFIGHT.MAN";
// const EXAMPLE_ANIMATION: &str = "zengin://_WORK/DATA/ANIMS/_COMPILED/HUMANS-S_RUN.MAN";
// const EXAMPLE_ANIMATION: &str = "zengin://_WORK/DATA/ANIMS/_COMPILED/HUMANS-S_RUNL.MAN";

/// Check only entities which were not handled previously
#[derive(Component, Default)]
struct ObjectEntitiesSpawned {}

#[derive(Component, Default)]
struct NpcSpawnState {
    body_handle: Handle<ZenGinModel>,
    armor_handle: Option<Handle<ZenGinModel>>,
    animation_handle: Option<Handle<AnimationData>>,
    body_spawned: bool,
    head_spawned: bool,
    armor_spawned: bool,
}

pub(crate) fn create_skinned_mesh_data(
    commands: &mut Commands,
    material_handles: &mut MaterialHandles,
    skinned_mesh_inverse_bindposes_assets: &mut Assets<SkinnedMeshInverseBindposes>,
    model_handle: &Handle<ZenGinModel>,
    skeleton_parent: Entity,
    model: &ZenGinModel,
    animation_data: Option<&AnimationData>,
) -> Option<SkinnedMesh> {
    let is_skinned = model.sub_meshes.iter().any(|el| el.is_skinned);
    if !is_skinned {
        return None;
    }

    // Find node with name BIP01
    let base_pos_index = model
        .node_names
        .iter()
        .position(|el| el == "BIP01")
        .unwrap_or(0);

    let animation_data = Arc::new(animation_data.cloned().unwrap_or(AnimationData::default()));

    let frame_state = Arc::new(FrameState {
        start_frame: AtomicU32::new(0),
        end_frame: AtomicU32::new(1),
        mul_1000: AtomicU32::new(0),
        needs_update: AtomicBool::new(true),
    });

    let add_animation = animation_data.is_good_animations();

    if add_animation {
        commands.entity(skeleton_parent).insert(AnimationState {
            frame_state: frame_state.clone(),
            fps: animation_data.fps,
            frames_num: animation_data.frames_num as usize,
            time_us: AtomicU32::new(0),
            movement: animation_data.root_movement(base_pos_index),
            base_pos_node_index: base_pos_index,
        });
    }

    let joint_entities: Vec<_> = model
        .nodes
        .iter()
        .enumerate()
        .map(|(index, el)| {
            let mut ent_cmd = commands.spawn((AnimatedJoint, *el, Visibility::Inherited));

            if add_animation {
                ent_cmd.insert(BoneAnimationData {
                    skeleton: skeleton_parent,
                    node_index: index,
                    is_base_pos_bone: index == base_pos_index,
                    bone_index: animation_data.get_index_for_bone(index),
                    animation_data: animation_data.clone(),
                    frame_state: frame_state.clone(),
                });
            }

            ent_cmd.id()
        })
        .collect();

    for (index, ent) in joint_entities.iter().enumerate() {
        let parent = model.parents[index];
        if parent < 0 {
            commands.entity(skeleton_parent).add_child(*ent);
            continue;
        }
        commands
            .entity(joint_entities[parent as usize])
            .add_child(*ent);
    }

    let inverse_bindposes = material_handles.get_inverse_bindposes_handle(
        skinned_mesh_inverse_bindposes_assets,
        model,
        model_handle,
    );
    assert!(!model.inverse_bindposes.is_empty());
    assert!(!joint_entities.is_empty());
    assert!(joint_entities.len() == model.inverse_bindposes.len());
    return Some(SkinnedMesh {
        inverse_bindposes,
        joints: joint_entities,
    });
}

#[allow(clippy::type_complexity)]
fn spawn_npcs(
    models: ResMut<Assets<ZenGinModel>>,
    animations: ResMut<Assets<AnimationData>>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
    mut handles_map: ResMut<MaterialHandles>,
    mut query: Query<
        (
            Entity,
            &GameNpc,
            &NpcModel,
            &NpcAnimations,
            Option<&mut NpcSpawnState>,
        ),
        (With<DistanceNear>, Without<ObjectEntitiesSpawned>),
    >,
    mut skinned_mesh_inverse_bindposes_assets: ResMut<Assets<SkinnedMeshInverseBindposes>>,
) {
    for (entity_id, game_npc, npc_model, npc_animations, spawn_state) in &mut query {
        let Some(mut spawn_state) = spawn_state else {
            // Plays the configured animation, otherwise the standing animation
            // of the model scheme (e.g. a wolf stands in `WOLF-S_FISTRUN`)
            let animation_path = npc_animations
                .animation
                .clone()
                .or_else(|| {
                    let scheme = npc_model.animation_scheme.as_deref()?;
                    let stand = npc_model.stand_animation.as_deref()?;
                    Some(get_animation_path(scheme, stand))
                })
                .unwrap_or_else(|| {
                    warn_once!(
                        "NPC({}), body({}) doesn't have idle animation",
                        game_npc.npc_instance_id,
                        &npc_model.body_model
                    );
                    EXAMPLE_ANIMATION.to_string()
                });
            let spawn_state = NpcSpawnState {
                body_handle: handles_map.get_model_handle(
                    &asset_server,
                    &npc_model.body_model,
                    npc_model.hierarchy.as_deref(),
                ),
                armor_handle: npc_model.armor_model.as_ref().map(|el| {
                    handles_map.get_model_handle(&asset_server, el, npc_model.hierarchy.as_deref())
                }),
                animation_handle: Some(
                    handles_map.get_animation_handle(&asset_server, &animation_path),
                ),
                body_spawned: npc_model.armor_model.is_some(),
                head_spawned: npc_model.head_model.is_none(),
                armor_spawned: npc_model.armor_model.is_none(),
            };

            let mut entity = commands.entity(entity_id);
            entity.insert(spawn_state);
            continue;
        };

        let body_load_state = asset_server.load_state(spawn_state.body_handle.id());

        if body_load_state.is_failed() {
            let mut entity = commands.entity(entity_id);
            entity.insert(ObjectEntitiesSpawned::default());
            // entity.insert(ClassifyDistance::default());
            continue;
        }

        let Some(model_data) = models.get(&spawn_state.body_handle) else {
            continue;
        };

        let armor_data = spawn_state
            .armor_handle
            .as_ref()
            .and_then(|el| models.get(el));
        if spawn_state.armor_handle.is_some() && armor_data.is_none() {
            continue;
        }
        let animation_data = spawn_state
            .animation_handle
            .as_ref()
            .and_then(|el| animations.get(el));
        if spawn_state.animation_handle.is_some() && animation_data.is_none() {
            continue;
        }

        {
            let mut entity = commands.entity(entity_id);
            entity.remove::<NpcSpawnState>();
            entity.insert(ObjectEntitiesSpawned::default());
        }

        let bones_data = create_skinned_mesh_data(
            &mut commands,
            &mut *handles_map,
            &mut skinned_mesh_inverse_bindposes_assets,
            &spawn_state.body_handle,
            entity_id,
            model_data,
            animation_data,
        );

        if let Some(fatness) = npc_model.model_fatness {
            apply_torso_fatness(&mut commands, model_data, bones_data.as_ref(), fatness);
        }

        // Record the engine weapon slot nodes (sheathed and drawn positions)
        // so the weapon system can attach/move the equipped weapons later.
        if let Some(bones) = bones_data.as_ref() {
            let find = |name: &str| {
                model_data
                    .node_names
                    .iter()
                    .position(|node| node == name)
                    .and_then(|index| bones.joints.get(index))
                    .copied()
            };
            let slots = NpcWeaponSlots {
                hand: find("ZS_RIGHTHAND"),
                left_hand: find("ZS_LEFTHAND"),
                sword: find("ZS_SWORD"),
                longsword: find("ZS_LONGSWORD"),
                bow: find("ZS_BOW"),
                crossbow: find("ZS_CROSSBOW"),
            };
            if slots.hand.is_some()
                || slots.left_hand.is_some()
                || slots.sword.is_some()
                || slots.longsword.is_some()
                || slots.bow.is_some()
                || slots.crossbow.is_some()
            {
                commands.entity(entity_id).insert(slots);
            }
        }

        if let Some(animation_handle) = spawn_state.animation_handle.clone() {
            commands
                .entity(entity_id)
                .insert(AnimationController::new(animation_handle));
        }
        let head_node_index = model_data
            .node_names
            .iter()
            .position(|el| el == "BIP01 HEAD");

        if let Some(head_model) = &npc_model.head_model {
            let tr = Transform::IDENTITY;

            let head_entity_id = if let Some(bones_data) = &bones_data
                && let Some(head_node_index) = head_node_index
            {
                bones_data.joints[head_node_index]
            } else {
                entity_id
            };

            let mut entity = commands.entity(head_entity_id);

            entity.with_child((
                Visibility::default(),
                ZenGinModelComponent {
                    model_handle: handles_map.get_model_handle(&asset_server, head_model, None),
                    override_texture: npc_model.head_texture.clone(),
                    ..default()
                },
                tr,
            ));
            spawn_state.head_spawned = true;
        }
        if !spawn_state.body_spawned {
            spawn_state.body_spawned = true;
            let tr = Transform::IDENTITY;

            let mut entity = commands.entity(entity_id);
            entity.with_child((
                Visibility::default(),
                ZenGinModelComponent {
                    model_handle: spawn_state.body_handle.clone(),
                    override_texture: npc_model.body_texture.clone(),
                    bones_data: bones_data.clone(),
                    ..default()
                },
                tr,
            ));
        }
        if let Some(armor_model) = &npc_model.armor_model {
            let armor_data = armor_data.unwrap();
            let bones_data_anim = bones_data.unwrap();

            let inverse_bindposes = handles_map.get_inverse_bindposes_handle(
                &mut skinned_mesh_inverse_bindposes_assets,
                armor_data,
                spawn_state.armor_handle.as_ref().unwrap(),
            );
            let bones_data = SkinnedMesh {
                inverse_bindposes,
                joints: armor_data
                    .animation_bone_index
                    .iter()
                    .map(|el| bones_data_anim.joints[*el])
                    .collect(),
            };

            let tr = Transform::IDENTITY;

            let mut entity = commands.entity(entity_id);
            entity.with_child((
                Visibility::default(),
                ZenGinModelComponent {
                    model_handle: handles_map.get_model_handle(
                        &asset_server,
                        armor_model,
                        npc_model.hierarchy.as_deref(),
                    ),
                    bones_data: Some(bones_data),
                    ..default()
                },
                tr,
                NpcVisibility::default(),
            ));
            spawn_state.armor_spawned = true;
        }
    }
}

fn add_classify_distance(mut commands: Commands, query: Query<Entity, Added<GameNpc>>) {
    for entity_id in query {
        commands
            .entity(entity_id)
            .insert(ClassifyDistance::default());
    }
}

/// Fattens the torso by scaling the spine joint. Skinned meshes are deformed by
/// their joints, so a joint has to be scaled (scaling the mesh entity cancels
/// out against the joint matrices). Bone-local axes are not world axes, so the
/// scale is uniform.
fn apply_torso_fatness(
    commands: &mut Commands,
    model: &ZenGinModel,
    bones: Option<&SkinnedMesh>,
    fatness: f32,
) {
    let Some(bones) = bones else {
        return;
    };
    let Some(index) = model
        .node_names
        .iter()
        .position(|name| name == "BIP01 SPINE")
    else {
        return;
    };
    let Some(joint) = bones.joints.get(index) else {
        return;
    };
    let scale = (1.0 + fatness * 0.1).max(0.1);
    let mut tr = model.nodes[index];
    tr.scale = Vec3::splat(scale);
    commands.entity(*joint).insert(tr);
}

fn despawn_visual_when_far(
    mut commands: Commands,
    q_parent: Query<Entity, (With<DistanceFar>, With<ObjectEntitiesSpawned>)>,
) {
    for entity_id in q_parent.iter() {
        let mut main_entity = commands.entity(entity_id);
        main_entity.remove::<ObjectEntitiesSpawned>();
        main_entity.despawn_children();
    }
}
