use std::sync::Arc;
use std::sync::atomic::AtomicU32;

use crate::game::distance::DistanceNear;
use crate::gui::toggle_visibility::show_gizmos;
use crate::zengin::common::ZenGinModel;
use crate::zengin::loaders::animation::AnimationData;
use avian3d::prelude::LinearVelocity;
use bevy::mesh::skinning::{SkinnedMesh, SkinnedMeshInverseBindposes};
use bevy::prelude::*;

#[derive(Default)]
pub struct SkeletalAnimations;

impl Plugin for SkeletalAnimations {
    fn build(&self, app: &mut App) {
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

#[derive(Component)]
pub struct AnimatedJoint;

#[derive(Component, Debug)]
pub struct BoneAnimationData {
    pub animation_data: Arc<AnimationData>,
    pub bone_index: usize,
    pub is_base_pos_bone: bool,
    pub frame_state: Arc<FrameState>,
}

#[derive(Debug)]
pub struct FrameState {
    pub start_frame: AtomicU32,
    pub end_frame: AtomicU32,
    pub mul_1000: AtomicU32,
    pub mov_delta_1000: AtomicU32,
}

#[derive(Debug)]
pub struct AnimateBasedOnMovement {
    pub movement: Vec<f32>,
    pub delta_movement: AtomicU32,
}

#[derive(Debug)]
pub struct StandardAnimation {
    pub time_ms: AtomicU32,
    pub fps: f32,
}

#[derive(Component, Debug)]
pub struct AnimateBasedOnMovementComponent(pub Arc<FrameState>, pub AnimateBasedOnMovement);

#[derive(Component, Debug)]
pub struct StandardAnimationComponent {
    pub frame_state: Arc<FrameState>,
    pub model_animation_state: StandardAnimation,
    pub frames_num: u32,
}

pub fn create_skinned_mesh_data(
    commands: &mut Commands,
    skinned_mesh_inverse_bindposes_assets: &mut ResMut<Assets<SkinnedMeshInverseBindposes>>,
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

    let animation_data = animation_data.cloned().unwrap_or(AnimationData::default());
    let has_movement = animation_data.bone_has_movement(base_pos_index);

    let frame_state = Arc::new(FrameState {
        start_frame: AtomicU32::new(1),
        end_frame: AtomicU32::new(1),
        mul_1000: AtomicU32::new(1),
        mov_delta_1000: AtomicU32::new(1),
    });

    let animation_data = Arc::new(animation_data);
    if has_movement {
        let model_animation_state = AnimateBasedOnMovement {
            movement: animation_data.get_movement(base_pos_index),
            delta_movement: AtomicU32::new(1),
        };
        commands
            .entity(skeleton_parent)
            .insert(AnimateBasedOnMovementComponent(
                frame_state.clone(),
                model_animation_state,
            ));
    } else {
        let model_animation_state = StandardAnimation {
            fps: animation_data.fps,
            time_ms: AtomicU32::new(1),
        };
        commands
            .entity(skeleton_parent)
            .insert((StandardAnimationComponent {
                frame_state: frame_state.clone(),
                model_animation_state,
                frames_num: animation_data.frames_num,
            },));
    }

    let joint_entities: Vec<_> = model
        .nodes
        .iter()
        .enumerate()
        .map(|(index, el)| {
            if let Some(local_index) = animation_data.get_index_for_bone(index) {
                commands
                    .spawn((
                        AnimatedJoint,
                        BoneAnimationData {
                            animation_data: animation_data.clone(),
                            bone_index: local_index,
                            is_base_pos_bone: index == base_pos_index,
                            frame_state: frame_state.clone(),
                        },
                        *el,
                        Visibility::Inherited,
                    ))
                    .id()
            } else {
                commands
                    .spawn((AnimatedJoint, *el, Visibility::Inherited))
                    .id()
            }
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

    let inverse_bindposes =
        skinned_mesh_inverse_bindposes_assets.add(model.inverse_bindposes.clone());
    assert!(!model.inverse_bindposes.is_empty());
    assert!(!joint_entities.is_empty());
    assert!(joint_entities.len() == model.inverse_bindposes.len());
    return Some(SkinnedMesh {
        inverse_bindposes,
        joints: joint_entities,
    });
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

const ATOMIC_SCALE: f32 = 1000.0;
// Main animation bone contains object movement component from start of aniamtion
// We choose animation frame not on time but based on how much object have moved
// Object movement is not linear but we assume that object moves with constant speed this
//  results with object main bone having small viggle during aniamtion
#[allow(clippy::cast_precision_loss)]
fn update_frame_shared_data(data: &AnimateBasedOnMovementComponent, movement_in_frame: f32) {
    let shared = &data.1;
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
    let start_frame = start_frame % frames_num; // Hopefully this fixes: index out of bounds: the len is 20 but the index is 21
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
    let frame_state = &data.0;
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

fn update_animation_standard(
    time: Res<Time>,
    query: Query<&StandardAnimationComponent, With<DistanceNear>>,
) {
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
