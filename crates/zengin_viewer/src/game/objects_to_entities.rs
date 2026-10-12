use crate::game::animation::{AnimationController, LocomotionSequencer, create_skinned_mesh_data};
use crate::game::distance::{ClassifyDistance, DistanceFar, DistanceNear};
use crate::game::objects::{GameNpc, NpcAnimations, NpcModel};
use crate::gui::toggle_visibility::NpcVisibility;
use crate::warn_once;
use crate::zengin::common::ZenGinModel;
use crate::zengin::loaders::animation::AnimationData;
use crate::zengin_resources::{
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
    movement_handles: Option<MovementHandles>,
    body_spawned: bool,
    head_spawned: bool,
    armor_spawned: bool,
}

/// Loaded [`MovementAnimations`]
#[derive(Clone)]
struct MovementHandles {
    idle: Handle<AnimationData>,
    move_start: Option<Handle<AnimationData>>,
    move_loop: Handle<AnimationData>,
    move_stop: Option<Handle<AnimationData>>,
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
            let movement_handles =
                npc_animations
                    .movement_animations
                    .as_ref()
                    .map(|el| MovementHandles {
                        idle: handles_map.get_animation_handle(&asset_server, &el.idle),
                        move_start: el
                            .move_start
                            .as_ref()
                            .map(|path| handles_map.get_animation_handle(&asset_server, path)),
                        move_loop: handles_map.get_animation_handle(&asset_server, &el.move_loop),
                        move_stop: el
                            .move_stop
                            .as_ref()
                            .map(|path| handles_map.get_animation_handle(&asset_server, path)),
                    });
            // Entities with movement animations start in their idle animation,
            // the rest plays the configured animation
            let animation_path = npc_animations
                .movement_animations
                .as_ref()
                .map(|el| el.idle.as_str())
                .or(npc_animations.animation.as_deref());
            let animation_path = if let Some(animation_path) = animation_path {
                animation_path
            } else {
                warn_once!(
                    "NPC({}), body({}) doesn't have idle animation",
                    game_npc.npc_instance_id,
                    &npc_model.body_model
                );
                EXAMPLE_ANIMATION
            };
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
                    handles_map.get_animation_handle(&asset_server, animation_path),
                ),
                movement_handles,
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
            &mut skinned_mesh_inverse_bindposes_assets,
            entity_id,
            model_data,
            animation_data,
        );

        if let Some(animation_handle) = spawn_state.animation_handle.clone() {
            commands.entity(entity_id).insert(AnimationController {
                current: animation_handle,
                next: None,
                interrupt_current: false,
            });
            // Entities with movement animations get a locomotion sequencer
            // driving their controller, the rest just loops its single animation
            if let Some(movement) = spawn_state.movement_handles.clone() {
                commands.entity(entity_id).insert(LocomotionSequencer {
                    idle: movement.idle,
                    move_start: movement.move_start,
                    move_loop: movement.move_loop,
                    move_stop: movement.move_stop,
                });
            }
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

            let inverse_bindposes =
                skinned_mesh_inverse_bindposes_assets.add(armor_data.inverse_bindposes.clone());
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
