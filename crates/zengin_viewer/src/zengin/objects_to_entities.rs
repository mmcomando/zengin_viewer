use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32};

use crate::gameplay::distance::{ClassifyDistance, DistanceFar, DistanceNear};
use crate::gameplay::objects::{NpcBodyRootOffset, NpcModel};
use crate::graphics::{
    AnimatedJoint, AnimationController, AnimationData, AnimationState, BoneAnimationData,
    FrameState, NpcAnimations,
};
use crate::warn_once;
use crate::zengin::GameNpc;
use crate::zengin::common::get_animation_path;
use crate::zengin::interaction::selection::spawn_npc_selection_collider;
use crate::zengin::npc_routines::NpcWeaponSlots;
use crate::zengin::resources::{
    CachedAssetHandles, ZenGinModelComponent, despawn_visual_when_far_resource,
};
use crate::zengin::visual::model::ZenGinModel;
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

pub(crate) fn create_skinned_mesh_data(
    commands: &mut Commands,
    material_handles: &mut CachedAssetHandles,
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
        root_motion: AtomicBool::new(false),
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
    assert_eq!(joint_entities.len(), model.inverse_bindposes.len());
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
    mut handles_map: ResMut<CachedAssetHandles>,
    mut query: Query<
        (
            Entity,
            &GameNpc,
            &NpcModel,
            &NpcAnimations,
            Option<&AnimationController>,
        ),
        (With<DistanceNear>, Without<ObjectEntitiesSpawned>),
    >,
    mut skinned_mesh_inverse_bindposes_assets: ResMut<Assets<SkinnedMeshInverseBindposes>>,
) {
    for (entity_id, game_npc, npc_model, npc_animations, existing_controller) in &mut query {
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
                    game_npc.instance_id(),
                    &npc_model.body_model
                );
                EXAMPLE_ANIMATION.to_string()
            });
        let body_handle = handles_map.get_model_handle(
            &asset_server,
            &npc_model.body_model,
            npc_model.hierarchy.as_deref(),
        );
        let animation_handle = handles_map.get_animation_handle(&asset_server, &animation_path);

        if asset_server.load_state(body_handle.id()).is_failed() {
            let mut entity = commands.entity(entity_id);
            entity.insert(ObjectEntitiesSpawned::default());
            spawn_npc_selection_collider(&mut commands, entity_id);
            continue;
        }

        let Some(model_data) = models.get(&body_handle) else {
            continue;
        };
        let Some(animation_data) = animations.get(&animation_handle) else {
            continue;
        };

        {
            let mut entity = commands.entity(entity_id);
            entity.insert(ObjectEntitiesSpawned::default());
            spawn_npc_selection_collider(&mut commands, entity_id);
        }

        let bones_data = create_skinned_mesh_data(
            &mut commands,
            &mut handles_map,
            &mut skinned_mesh_inverse_bindposes_assets,
            &body_handle,
            entity_id,
            model_data,
            Some(animation_data),
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

        // A re-spawned visual (e.g. after the far-distance despawn) must
        // not reset playback state: re-inserting over an existing
        // component does not fire `Added`, so the player's sequencer
        // would never re-enable root motion and the character would stop
        // being moved by animations. Preserve the controller state.
        let controller = if let Some(existing) = existing_controller {
            let mut controller = AnimationController::new(existing.current().clone());
            if let Some(next) = existing.next() {
                controller.queue_next(next.clone(), existing.interrupt_current());
            }
            controller.set_root_motion(existing.root_motion());
            controller
        } else {
            AnimationController::new(animation_handle)
        };
        commands.entity(entity_id).insert(controller);
        commands
            .entity(entity_id)
            .insert(NpcBodyRootOffset(model_data.root_translation.y));
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
        }
        let tr = Transform::IDENTITY;

        let body = commands.spawn((
            Visibility::default(),
            ZenGinModelComponent {
                model_handle: body_handle.clone(),
                override_texture: npc_model.body_texture.clone(),
                bones_data: bones_data.clone(),
                ..default()
            },
            tr,
            crate::zengin::npc_routines::NpcBodyMarker,
        ));
        let body = body.id();
        commands.entity(entity_id).add_child(body);
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
