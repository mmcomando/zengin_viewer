use crate::game::objects::GameNpc;
use crate::toggle_visibility::{NpcVisibility, show_gizmos};
use crate::zengin::common::ZenGinModel;
use crate::zengin::loaders::animation::AnimationData;
use crate::zengin_resources::{
    AnimateBasedOnMovementComponent, BoneAnimationData, MaterialHandles,
    StandardAnimationComponent, ZenGinModelComponent, create_skined_mesh_data,
};
use avian3d::prelude::LinearVelocity;
use bevy::mesh::skinning::{SkinnedMesh, SkinnedMeshInverseBindposes};
use bevy::prelude::*;

#[derive(Default)]
pub struct GameObjectSpawnEntities;

impl Plugin for GameObjectSpawnEntities {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, object_to_entities);
        app.add_systems(Update, draw_bones.run_if(show_gizmos));
        app.add_systems(Update, update_animation_based_on_movement);
        app.add_systems(Update, update_animation_standard);
        app.add_systems(
            Update,
            compute_animations
                .after(update_animation_based_on_movement)
                .after(update_animation_standard),
        );
    }
}

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

#[derive(Component)]
pub struct AnimatedJoint;

#[allow(clippy::type_complexity)]
fn object_to_entities(
    models: ResMut<Assets<ZenGinModel>>,
    animations: ResMut<Assets<AnimationData>>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
    mut handles_map: ResMut<MaterialHandles>,
    mut query: Query<
        (Entity, &GameNpc, Option<&mut NpcSpawnState>),
        Without<ObjectEntitiesSpawned>,
    >,
    mut skinned_mesh_inverse_bindposes_assets: ResMut<Assets<SkinnedMeshInverseBindposes>>,
) {
    for (entity_id, npc_component, spawn_state) in &mut query {
        let Some(mut spawn_state) = spawn_state else {
            let spawn_state = NpcSpawnState {
                body_handle: handles_map.get_model_handle(
                    &asset_server,
                    &npc_component.body_model,
                    npc_component.hierarchy.as_deref(),
                ),
                armor_handle: npc_component.armor_model.as_ref().map(|el| {
                    handles_map.get_model_handle(
                        &asset_server,
                        el,
                        npc_component.hierarchy.as_deref(),
                    )
                }),
                animation_handle: Some(
                    handles_map.get_animation_handle(
                        &asset_server,
                        npc_component
                            .animation
                            .as_ref()
                            .map_or(EXAMPLE_ANIMATION, |el| el.as_str()),
                    ),
                ),
                body_spawned: npc_component.armor_model.is_some(),
                head_spawned: npc_component.head_model.is_none(),
                armor_spawned: npc_component.armor_model.is_none(),
            };

            let mut entity = commands.entity(entity_id);
            entity.insert(spawn_state);
            continue;
        };

        let body_load_state = asset_server.load_state(spawn_state.body_handle.id());

        if body_load_state.is_failed() {
            let mut entity = commands.entity(entity_id);
            entity.insert(ObjectEntitiesSpawned::default());
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
            entity.insert(ObjectEntitiesSpawned::default());
        }

        let bones_data = create_skined_mesh_data(
            &mut commands,
            &mut skinned_mesh_inverse_bindposes_assets,
            entity_id,
            model_data,
            animation_data,
        );
        let head_node_index = model_data
            .node_names
            .iter()
            .position(|el| el == "BIP01 HEAD");

        if let Some(head_model) = &npc_component.head_model {
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
                    override_texture: npc_component.head_texture.clone(),
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
                    override_texture: npc_component.body_texture.clone(),
                    bones_data: bones_data.clone(),
                    ..default()
                },
                tr,
            ));
        }
        if let Some(armor_model) = &npc_component.armor_model {
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
                        npc_component.hierarchy.as_deref(),
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

fn draw_bones(
    mut gizmos: Gizmos,

    bones: Query<(Entity, &GlobalTransform, Option<&ChildOf>), With<AnimatedJoint>>,
    transforms: Query<&GlobalTransform>,
) {
    for (_bone_entity, bone_transform, parent) in bones.iter() {
        if let Some(parent) = parent
            && let Ok(parent_transform) = transforms.get(parent.0)
        {
            // Draw a line from parent bone to current bone
            gizmos.line(
                parent_transform.translation(),
                bone_transform.translation(),
                Color::srgb(0.0, 1.0, 0.0), // Green bones
            );

            // Optional: Draw the bone itself (a small sphere)
            // gizmos.sphere(
            //     bone_transform.translation(),
            //     // Quat::IDENTITY,
            //     0.02,
            //     Color::srgb(1.0, 0.0, 0.0), // Red joints
            // );
        }
    }
}

// const EXAMPLE_ANIMATION: &str = "zengin://_WORK/DATA/ANIMS/_COMPILED/HUMANS-T_YES.MAN";
// const EXAMPLE_ANIMATION: &str = "zengin://_WORK/DATA/ANIMS/_COMPILED/HUMANS-S_WALKL.MAN";
// const EXAMPLE_ANIMATION: &str = "zengin://_WORK/DATA/ANIMS/_COMPILED/HUMANS-S_WALKWL.MAN";
const EXAMPLE_ANIMATION: &str = "zengin://_WORK/DATA/ANIMS/_COMPILED/HUMANS-S_WASH.MAN";
// const EXAMPLE_ANIMATION: &str = "zengin://_WORK/DATA/ANIMS/_COMPILED/HUMANS-S_WATCHFIGHT.MAN";
// const EXAMPLE_ANIMATION: &str = "zengin://_WORK/DATA/ANIMS/_COMPILED/HUMANS-S_RUN.MAN";
// const EXAMPLE_ANIMATION: &str = "zengin://_WORK/DATA/ANIMS/_COMPILED/HUMANS-S_RUNL.MAN";

const ATOMIC_SCALE: f32 = 1000.0;
// Main animation bone contains object movement component from start of aniamtion
// We choose animation frame not on time but based on how much object have moved
// Object movement is not linear but we assume that object moves with constant speed this
//  results with object main bone having small viggle during aniamtion
#[allow(clippy::cast_precision_loss)]
fn update_frame_shared_data(data: &AnimateBasedOnMovementComponent, movement_in_frame: f32) {
    let shared = &data.model_animation_state;
    let movement = &shared.movement;
    let frames_num = movement.len();
    let frames_num_f = movement.len() as f32;

    let mut accumulated_movement = shared
        .delta_movement
        .load(std::sync::atomic::Ordering::Acquire) as f32
        / ATOMIC_SCALE
        + movement_in_frame;

    // We extrapolate movement for the last frame so interpolation can wrap back to frame 0.
    let total_time = (movement.last().unwrap()) * (1.0 + 1.0 / frames_num_f);
    let mut animation_progress = (accumulated_movement / total_time) * frames_num_f;
    if animation_progress > frames_num_f {
        animation_progress -= frames_num_f;
        accumulated_movement -= total_time;
    }

    let frame_factor = animation_progress % 1.0;
    let start_frame = animation_progress as usize;
    let end_frame = (start_frame + 1) % frames_num;

    let start_movement = movement[start_frame];
    let end_movement = movement[end_frame];
    let frame_movement = if end_movement >= start_movement {
        end_movement - start_movement
    } else {
        total_time - start_movement
    };
    let mov_delta = start_movement - frame_movement * frame_factor - accumulated_movement;

    // Convert to integers so we can use atomics
    let delta_movement = (accumulated_movement * ATOMIC_SCALE) as u32;
    let frame_factor_1000 = (frame_factor * ATOMIC_SCALE) as u32;
    let mov_delta = (mov_delta * ATOMIC_SCALE) as u32;

    // Save results using atomics so Rust allows shared access without locks
    shared
        .delta_movement
        .store(delta_movement, std::sync::atomic::Ordering::Release);
    let frame_state = &data.frame_state;
    frame_state
        .start_frame
        .store(start_frame as u32, std::sync::atomic::Ordering::Release);
    frame_state
        .end_frame
        .store(end_frame as u32, std::sync::atomic::Ordering::Release);
    frame_state
        .mul_1000
        .store(frame_factor_1000, std::sync::atomic::Ordering::Release);
    frame_state
        .mov_delta_1000
        .store(mov_delta, std::sync::atomic::Ordering::Release);
}

fn update_animation_based_on_movement(
    time: Res<Time>,
    query: Query<(&ChildOf, &AnimateBasedOnMovementComponent)>,
    q_vel: Query<&LinearVelocity>,
) {
    let delta = time.delta_secs();
    for (parent, data) in &query {
        let vel = q_vel.get(parent.parent()).unwrap();
        let movement_in_frame = delta * vel.length();
        update_frame_shared_data(data, movement_in_frame);
    }
}

#[allow(clippy::cast_precision_loss)]
fn update_frame_standard_animation(data: &StandardAnimationComponent, dt_time: f32) {
    let shared = &data.model_animation_state;

    let frames_num = data.frames_num as usize;
    let frames_num_f = frames_num as f32;

    let mut accumulated_time =
        shared.time_ms.load(std::sync::atomic::Ordering::Acquire) as f32 / ATOMIC_SCALE + dt_time;

    let one_frame_time = 1.0 / data.model_animation_state.fps;

    // We extrapolate movement for the last frame so interpolation can wrap back to frame 0.
    let total_time = one_frame_time * (frames_num_f + 1.0);
    let mut animation_progress = (accumulated_time / total_time) * frames_num_f;
    if animation_progress > frames_num_f {
        animation_progress -= frames_num_f;
        accumulated_time -= total_time;
    }

    let frame_factor = animation_progress % 1.0;
    let start_frame = animation_progress as usize;
    let end_frame = (start_frame + 1) % frames_num;

    // Convert to integers so we can use atomics
    let frame_factor_1000 = (frame_factor * ATOMIC_SCALE) as u32;
    let time_ms = (accumulated_time * ATOMIC_SCALE) as u32;

    // Save results using atomics so Rust allows shared access without locks
    shared
        .time_ms
        .store(time_ms, std::sync::atomic::Ordering::Release);
    let frame_state = &data.frame_state;
    frame_state
        .start_frame
        .store(start_frame as u32, std::sync::atomic::Ordering::Release);
    frame_state
        .end_frame
        .store(end_frame as u32, std::sync::atomic::Ordering::Release);
    frame_state
        .mul_1000
        .store(frame_factor_1000, std::sync::atomic::Ordering::Release);
    frame_state
        .mov_delta_1000
        .store(0, std::sync::atomic::Ordering::Release);
}

fn update_animation_standard(time: Res<Time>, query: Query<&StandardAnimationComponent>) {
    let delta = time.delta_secs();
    for data in &query {
        update_frame_standard_animation(data, delta);
    }
}

#[allow(clippy::cast_precision_loss)]
fn compute_animations(mut query: Query<(&mut Transform, &BoneAnimationData)>) {
    for (mut transform, data) in &mut query {
        let shared = &data.frame_state;
        let frame_num = shared
            .start_frame
            .load(std::sync::atomic::Ordering::Acquire) as usize;

        let end_frame = shared.end_frame.load(std::sync::atomic::Ordering::Acquire) as usize;

        let mul = shared.mul_1000.load(std::sync::atomic::Ordering::Acquire) as f32 / ATOMIC_SCALE;

        let mov_delta = shared
            .mov_delta_1000
            .load(std::sync::atomic::Ordering::Acquire) as f32
            / ATOMIC_SCALE;

        let frame_a = data
            .animation_data
            .get_bone_sample(frame_num, data.bone_index);

        let frame_b = data
            .animation_data
            .get_bone_sample(end_frame, data.bone_index);

        transform.rotation = frame_a
            .rotation
            .inverse()
            .lerp(frame_b.rotation.inverse(), mul);
        transform.translation = frame_a.position.lerp(frame_b.position, mul);

        if data.is_base_pos_bone {
            transform.translation.x = 0.0;
            transform.translation.z = -mov_delta;
        }
    }
}
