use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::character::move_and_slide;
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

        // Root movement has to be applied before the character controller,
        // so move-and-slide starts from the new position and can react to it.
        app.add_systems(
            Update,
            (update_animations.before(move_and_slide), compute_animations).chain(),
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

/// Interpolation state shared between the skeleton and all of its joints.
#[derive(Debug)]
pub struct FrameState {
    pub start_frame: AtomicU32,
    pub end_frame: AtomicU32,
    pub mul_1000: AtomicU32,
}

/// Plays an animation at its natural pace (`fps`).
///
/// When the animation moves the base pos bone, [`AnimationState::movement`] holds
/// that root movement and the owning entity is moved by it.
#[derive(Component, Debug)]
pub struct AnimationState {
    pub frame_state: Arc<FrameState>,
    pub fps: f32,
    pub frames_num: usize,
    /// Elapsed animation time in microseconds, always kept within one loop.
    pub time_us: AtomicU32,
    /// Forward displacement of the base pos bone for every frame, relative to frame 0.
    /// `None` when the animation does not move the base pos bone.
    pub movement: Option<Vec<f32>>,
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

    let frame_state = Arc::new(FrameState {
        start_frame: AtomicU32::new(0),
        end_frame: AtomicU32::new(1),
        mul_1000: AtomicU32::new(0),
    });

    let movement = if animation_data.bone_has_movement(base_pos_index) {
        Some(animation_data.get_movement(base_pos_index))
    } else {
        None
    };

    commands.entity(skeleton_parent).insert(AnimationState {
        frame_state: frame_state.clone(),
        fps: animation_data.fps,
        frames_num: animation_data.frames_num as usize,
        time_us: AtomicU32::new(0),
        movement,
    });

    let animation_data = Arc::new(animation_data);
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
/// Animation time is stored scaled to integers so it can be shared without locks.
const TIME_SCALE: f32 = 1_000_000.0;

/// Result of advancing one animation by one time step.
struct FrameUpdate {
    start_frame: usize,
    end_frame: usize,
    mul_1000: u32,
    time_us: u32,
    /// How much root movement the entity should be moved by this step.
    delta_movement: f32,
}

/// Advances an animation by `delta` seconds on its natural pace
/// (`frames_num / fps` per loop).
///
/// When the animation has root movement data, the movement curve difference between
/// the previous and the current progress is returned, so the entity travels exactly
/// as much as the animation moves its base pos bone.
#[allow(clippy::cast_precision_loss)]
fn advance_animation(data: &AnimationState, delta: f32) -> Option<FrameUpdate> {
    if data.fps <= 0.0 || data.frames_num < 2 {
        return None;
    }
    let frames_num = data.frames_num;
    let loop_time = frames_num as f32 / data.fps;

    let old_time = data.time_us.load(Ordering::Acquire) as f32 / TIME_SCALE;
    let raw_time = old_time + delta;
    let loops = (raw_time / loop_time) as u32;
    let new_time = raw_time % loop_time;

    let progress = new_time * data.fps;
    let start_frame = (progress as usize).min(frames_num - 1);

    let delta_movement = data.movement.as_deref().map_or(0.0, |movement| {
        // Movement past the last frame is extrapolated so looping keeps moving forward
        // instead of sliding back to the frame 0 position.
        let total =
            movement[frames_num - 1] + (movement[frames_num - 1] - movement[frames_num - 2]);
        let old_displacement = root_displacement(movement, total, old_time * data.fps);
        let new_displacement = root_displacement(movement, total, new_time * data.fps);
        if loops == 0 {
            new_displacement - old_displacement
        } else {
            // Animation wrapped around: finish the old loop, skip the full loops, start the new one
            (total - old_displacement) + total * ((loops - 1) as f32) + new_displacement
        }
    });

    Some(FrameUpdate {
        start_frame,
        end_frame: (start_frame + 1) % frames_num,
        mul_1000: (progress.fract() * ATOMIC_SCALE) as u32,
        time_us: (new_time * TIME_SCALE) as u32,
        delta_movement,
    })
}

/// Root forward displacement at animation `progress` (`0..frames_num`).
/// The segment past the last frame interpolates towards the extrapolated
/// full loop displacement (`total`).
fn root_displacement(movement: &[f32], total: f32, progress: f32) -> f32 {
    let segment = (progress as usize).min(movement.len() - 1);
    let factor = progress - segment as f32;
    let start = movement[segment];
    let end = movement.get(segment + 1).copied().unwrap_or(total);
    start + (end - start) * factor
}

/// Applies animations to entities. Animations play purely on time; when an animation
/// moves its base pos bone, the entity itself is moved by that root movement.
fn update_animations(
    time: Res<Time>,
    query: Query<(Entity, Option<&ChildOf>, &GlobalTransform, &AnimationState), With<DistanceNear>>,
    q_vel: Query<&LinearVelocity>,
    mut transforms: Query<&mut Transform>,
) {
    let delta = time.delta_secs();

    for (entity, parent, global_transform, data) in &query {
        let Some(update) = advance_animation(data, delta) else {
            continue;
        };

        let frame_state = &data.frame_state;
        frame_state
            .start_frame
            .store(update.start_frame as u32, Ordering::Release);
        frame_state
            .end_frame
            .store(update.end_frame as u32, Ordering::Release);
        frame_state
            .mul_1000
            .store(update.mul_1000, Ordering::Release);
        data.time_us.store(update.time_us, Ordering::Release);

        if update.delta_movement == 0.0 {
            continue;
        }

        // The animation state lives on the model entity. Characters with a physics body
        // own the model as a child, so move the physics entity, otherwise move the model itself.
        let target = match parent {
            Some(parent) if q_vel.contains(parent.parent()) => parent.parent(),
            _ => entity,
        };
        let Ok(mut transform) = transforms.get_mut(target) else {
            continue;
        };
        transform.translation += global_transform.rotation() * Vec3::Z * update.delta_movement;
    }
}

#[allow(clippy::cast_precision_loss)]
fn compute_animations(mut query: Query<(&mut Transform, &BoneAnimationData)>) {
    for (mut transform, data) in &mut query {
        let shared = &data.frame_state;
        let frame_num = shared.start_frame.load(Ordering::Acquire) as usize;

        let end_frame = shared.end_frame.load(Ordering::Acquire) as usize;

        let mul = shared.mul_1000.load(Ordering::Acquire) as f32 / ATOMIC_SCALE;

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
            // Root movement is applied to the entity by update_animations
            transform.translation.x = 0.0;
            transform.translation.z = 0.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Root movement dumped from the real `HUMANS-S_RUNL.MAN` (20 frames, 25 fps).
    fn runl_movement() -> Vec<f32> {
        vec![
            0.0, 0.13519135, 0.27853653, 0.41702682, 0.5482346, 0.6780731, 0.8079115, 0.9398663,
            1.072817, 1.2036511, 1.3291327, 1.4368128, 1.5380194, 1.655036, 1.8134439, 1.9851096,
            2.1610076, 2.3322375, 2.4898987, 2.6249032,
        ]
    }

    fn runl_state(movement: Option<Vec<f32>>) -> AnimationState {
        AnimationState {
            frame_state: Arc::new(FrameState {
                start_frame: AtomicU32::new(0),
                end_frame: AtomicU32::new(1),
                mul_1000: AtomicU32::new(0),
            }),
            fps: 25.0,
            frames_num: 20,
            time_us: AtomicU32::new(0),
            movement,
        }
    }

    #[test]
    fn plays_half_frame_at_natural_pace() {
        let state = runl_state(Some(runl_movement()));
        // Half of one frame time (1/25 s)
        let update = advance_animation(&state, 0.02).unwrap();

        assert_eq!(update.start_frame, 0);
        assert_eq!(update.end_frame, 1);
        assert!((update.mul_1000 as f32 / ATOMIC_SCALE - 0.5).abs() < 0.01);
        // Movement of half a frame: linear interpolation into the first frame step
        assert!((update.delta_movement - 0.5 * 0.13519135).abs() < 1e-4);
    }

    #[test]
    fn advances_one_frame_per_frame_time() {
        let state = runl_state(Some(runl_movement()));
        let update = advance_animation(&state, 1.0 / 25.0).unwrap();

        // Exactly one frame time lands on frame 1
        assert_eq!(update.start_frame, 1);
        assert_eq!(update.end_frame, 2);
        assert_eq!(update.mul_1000, 0);
        assert!((update.delta_movement - 0.13519135).abs() < 1e-3);
        assert!((update.time_us as f32 / TIME_SCALE - 0.04).abs() < 1e-5);
    }

    #[test]
    fn loop_travels_the_full_root_movement() {
        let movement = runl_movement();
        let total = movement[19] + (movement[19] - movement[18]);

        let state = runl_state(Some(movement));
        // Simulate one loop with 60 fps steps; travelled distance must match the
        // extrapolated full loop movement.
        let mut travelled = 0.0;
        for _ in 0..48 {
            let update = advance_animation(&state, 1.0 / 60.0).unwrap();
            state.time_us.store(update.time_us, Ordering::Release);
            travelled += update.delta_movement;
        }
        assert!((travelled - total).abs() < 1e-3, "travelled {travelled}");
    }

    #[test]
    fn long_pause_wraps_multiple_loops() {
        let movement = runl_movement();
        let total = movement[19] + (movement[19] - movement[18]);

        let state = runl_state(Some(movement.clone()));
        // 1.7 s is two loops (1.6 s) plus 0.1 s into the new loop
        let update = advance_animation(&state, 1.7).unwrap();

        let expected = 2.0 * total + movement[2] + (movement[3] - movement[2]) * 0.5;
        assert!((update.delta_movement - expected).abs() < 1e-3);
        assert_eq!(update.start_frame, 2);
    }

    #[test]
    fn animation_without_root_movement_does_not_move_entity() {
        let state = runl_state(None);
        let update = advance_animation(&state, 0.02).unwrap();

        assert_eq!(update.delta_movement, 0.0);
        assert_eq!(update.start_frame, 0);
    }

    #[test]
    fn invalid_animation_data_is_skipped() {
        let state = runl_state(None);
        let state = AnimationState {
            fps: 0.0,
            frames_num: 0,
            ..state
        };
        assert!(advance_animation(&state, 0.02).is_none());
    }
}
