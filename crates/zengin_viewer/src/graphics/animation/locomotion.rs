use std::sync::Arc;
use std::sync::atomic::Ordering;

use avian3d::prelude::LinearVelocity;
use bevy::prelude::*;

use crate::gameplay::distance::DistanceNear;

use super::components::{
    AnimationController, AnimationState, BoneAnimationData, LocomotionSequencer,
};
use super::data::AnimationData;

pub(crate) const ATOMIC_SCALE: f32 = 1000.0;
/// Animation time is stored scaled to integers so it can be shared without locks.
pub(crate) const TIME_SCALE: f32 = 1_000_000.0;
/// Horizontal velocity above which an entity counts as moving
const MOVEMENT_THRESHOLD_SQUARED: f32 = 0.5 * 0.5;

/// Result of advancing one animation by one time step.
pub(crate) struct FrameUpdate {
    pub(crate) start_frame: usize,
    pub(crate) end_frame: usize,
    pub(crate) mul_1000: u32,
    pub(crate) time_us: u32,
    /// How much root movement the entity should be moved by this step.
    #[allow(unused)]
    pub(crate) delta_movement: f32,
    /// Whether the animation completed at least one loop this step
    pub(crate) finished: bool,
}

/// Advances an animation by `delta` seconds on its natural pace
/// (`frames_num / fps` per loop).
///
/// When the animation has root movement data, the movement curve difference between
/// the previous and the current progress is returned, so the entity travels exactly
/// as much as the animation moves its base pos bone.
#[allow(clippy::cast_precision_loss)]
pub(crate) fn advance_animation(data: &AnimationState, delta: f32) -> Option<FrameUpdate> {
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
        finished: loops > 0,
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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum AnimationKind {
    Idle,
    MoveStart,
    MoveLoop,
    MoveStop,
}

impl AnimationKind {
    /// Looping animations may be interrupted immediately by a queued next
    /// animation, one-shot animations have to play to their end first.
    fn is_loop(self) -> bool {
        matches!(self, AnimationKind::Idle | AnimationKind::MoveLoop)
    }
}

/// Kind of a [`LocomotionSequencer`] animation. `None` for animations the
/// sequencer does not own (e.g. scripted actions), leaving their controller alone.
pub(crate) fn animation_kind(
    sequencer: &LocomotionSequencer,
    handle: &Handle<AnimationData>,
) -> Option<AnimationKind> {
    if *handle == sequencer.idle {
        Some(AnimationKind::Idle)
    } else if sequencer.move_start.as_ref().is_some_and(|el| el == handle) {
        Some(AnimationKind::MoveStart)
    } else if *handle == sequencer.move_loop {
        Some(AnimationKind::MoveLoop)
    } else if sequencer.move_stop.as_ref().is_some_and(|el| el == handle) {
        Some(AnimationKind::MoveStop)
    } else {
        None
    }
}

/// Decides which animation should follow the one matching `kind`, and whether
/// it may replace the current one immediately. `None` for the animation keeps
/// the current one looping.
pub(crate) fn desired_next(
    sequencer: &LocomotionSequencer,
    kind: AnimationKind,
    moving: bool,
) -> (Option<Handle<AnimationData>>, bool) {
    let next = match (moving, kind) {
        // Movement started or resumed: transition into the movement loop
        (true, AnimationKind::Idle | AnimationKind::MoveStop) => Some(
            sequencer
                .move_start
                .clone()
                .unwrap_or_else(|| sequencer.move_loop.clone()),
        ),
        // The start transition leads into the movement loop
        (true, AnimationKind::MoveStart) => Some(sequencer.move_loop.clone()),
        // Keep running
        (true, AnimationKind::MoveLoop) => None,
        // Movement stopped: transition back to idle
        (false, AnimationKind::MoveStart | AnimationKind::MoveLoop) => Some(
            sequencer
                .move_stop
                .clone()
                .unwrap_or_else(|| sequencer.idle.clone()),
        ),
        // The stop transition leads back to idle
        (false, AnimationKind::MoveStop) => Some(sequencer.idle.clone()),
        // Keep standing
        (false, AnimationKind::Idle) => None,
    };
    (next, kind.is_loop())
}

/// Whether the entity owning the animation is moving (characters own the
/// animation model as a child, their velocity drives the movement state)
fn is_moving(parent: Option<&ChildOf>, q_vel: &Query<&LinearVelocity>) -> bool {
    parent
        .and_then(|child_of| q_vel.get(child_of.parent()).ok())
        .is_some_and(|vel| vel.xz().length_squared() > MOVEMENT_THRESHOLD_SQUARED)
}

/// Bridges movement state and playback: entities with a [`LocomotionSequencer`]
/// switch between their movement animations based on entity movement. The
/// controller of an animation the sequencer does not own (e.g. a scripted
/// action) is left alone.
pub(crate) fn update_locomotion(
    mut query: Query<
        (
            Option<&ChildOf>,
            &LocomotionSequencer,
            &mut AnimationController,
        ),
        With<DistanceNear>,
    >,
    q_vel: Query<&LinearVelocity>,
) {
    for (parent, sequencer, mut controller) in &mut query {
        let Some(kind) = animation_kind(sequencer, &controller.current) else {
            continue;
        };
        let (next, interrupt) = desired_next(sequencer, kind, is_moving(parent, &q_vel));
        controller.next = next;
        controller.interrupt_current = interrupt;
    }
}

/// Applies animations to entities. Animations play purely on time; when an animation
/// moves its base pos bone, the entity itself is moved by that root movement.
/// Queued `next` animations are switched to when the current one ends or may
/// be interrupted (decided by [`update_locomotion`]).
#[allow(clippy::type_complexity)]
pub(crate) fn update_animations(
    time: Res<Time>,
    animations: Res<Assets<AnimationData>>,
    mut query: Query<
        (
            Entity,
            // Option<&ChildOf>,
            // &GlobalTransform,
            &mut AnimationState,
            Option<&mut AnimationController>,
        ),
        With<DistanceNear>,
    >,
    // q_vel: Query<&LinearVelocity>,
    // mut transforms: Query<&mut Transform>,
    mut joints: Query<&mut BoneAnimationData>,
) {
    let delta = time.delta_secs();
    let mut switches: Vec<(Entity, Arc<AnimationData>)> = Vec::new();

    for (
        entity,
        // parent, global_transform,
        mut data,
        controller,
    ) in &mut query
    {
        let Some(update) = advance_animation(&data, delta) else {
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

        // TODO: Improve movement by using update.delta_movement as animations know how much model should be moved to remove sliding
        // if update.delta_movement != 0.0 {
        //     // The animation state lives on the model entity. Characters with a physics body
        //     // own the model as a child, so move the physics entity, otherwise move the model itself.
        //     let target = match parent {
        //         Some(parent) if q_vel.contains(parent.parent()) => parent.parent(),
        //         _ => entity,
        //     };
        //     if let Ok(mut transform) = transforms.get_mut(target) {
        //         transform.translation +=
        //             global_transform.rotation() * Vec3::Z * update.delta_movement;
        //     }
        // }

        let Some(mut controller) = controller else {
            continue;
        };

        // // Uncomment to debug animations
        // if update.finished && controller.next.is_none() {
        //     info!("Loop animation({:?})", controller.current.path());
        // }

        // Switch when the current animation ended. Interruptible animations are
        // switched immediately, otherwise e.g. the 2s idle loop would delay
        // movement start.
        if (update.finished || controller.interrupt_current)
            && controller
                .next
                .as_ref()
                .is_some_and(|next| *next != controller.current)
            && let Some(next) = controller.next.take()
            && let Some(next_data) = animations.get(&next)
        {
            let next_data = Arc::new(next_data.clone());
            data.fps = next_data.fps;
            data.frames_num = next_data.frames_num as usize;
            data.movement = next_data.root_movement(data.base_pos_node_index);
            // New animation starts from its first frame
            data.time_us.store(0, Ordering::Release);
            data.frame_state.start_frame.store(0, Ordering::Release);
            data.frame_state.end_frame.store(1, Ordering::Release);
            data.frame_state.mul_1000.store(0, Ordering::Release);
            controller.current = next;
            switches.push((entity, next_data));
        }
    }

    // Joints of switched skeletons sample the new animation
    if !switches.is_empty() {
        for mut joint in &mut joints {
            let Some((_, new_data)) = switches
                .iter()
                .find(|(skeleton, _)| *skeleton == joint.skeleton)
            else {
                continue;
            };
            joint.animation_data = new_data.clone();
            joint.bone_index = new_data.get_index_for_bone(joint.node_index);
        }
    }
}

#[allow(clippy::cast_precision_loss)]
#[allow(clippy::type_complexity)]
pub(crate) fn compute_animations(mut query: Query<(&mut Transform, &BoneAnimationData)>) {
    // Bones of far objects are despawned together with the model root, so this
    // only ever sees joints of nearby skeletons. It is the per-bone inner loop
    // of skeletal animation, hence parallelized.
    query.par_iter_mut().for_each(|(mut transform, data)| {
        let Some(bone_index) = data.bone_index else {
            return;
        };

        let shared = &data.frame_state;
        let frame_num = shared.start_frame.load(Ordering::Acquire) as usize;

        let end_frame = shared.end_frame.load(Ordering::Acquire) as usize;

        let mul = shared.mul_1000.load(Ordering::Acquire) as f32 / ATOMIC_SCALE;

        let frame_a = data.animation_data.get_bone_sample(frame_num, bone_index);

        let frame_b = data.animation_data.get_bone_sample(end_frame, bone_index);

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
    });
}
