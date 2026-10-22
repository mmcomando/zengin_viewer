use std::sync::Arc;
use std::sync::atomic::Ordering;

use avian3d::prelude::LinearVelocity;
use bevy::{
    camera::primitives::{Aabb, Frustum, Sphere},
    prelude::*,
};

use super::animation_triggers::{AnimationTriggers, animation_name_from_path, crossed_triggers};
use crate::gameplay::character::{ACCELERATION_MULTIPLIER, AcceleratingRootMotion};
use crate::gameplay::distance::DistanceNear;
use crate::gameplay::objects::PlayerCharacter;

use super::components::{AnimationEnds, AnimationState, AnimationTrigger, BoneAnimationData};
use super::controller::AnimationController;
use super::data::AnimationData;

pub(crate) const ATOMIC_SCALE: f32 = 1000.0;
/// Animation time is stored scaled to integers so it can be shared without locks.
pub(crate) const TIME_SCALE: f32 = 1_000_000.0;

/// Result of advancing one animation by one time step.
pub(crate) struct FrameUpdate {
    pub(crate) start_frame: usize,
    pub(crate) end_frame: usize,
    pub(crate) mul_1000: u32,
    pub(crate) time_us: u32,
    /// How much root movement the entity should be moved by this step.
    pub(crate) delta_movement: f32,
    /// Whether the animation completed at least one loop this step.
    /// Consumers only distinguish "wrapped" from "not wrapped", the loop
    /// count itself is internal to the root movement computation.
    pub(crate) finished: bool,

    pub(crate) old_progress: f32,
    pub(crate) new_progress: f32,
}

/// Advances an animation by `delta` seconds on its natural pace
/// (`frames_num / fps` per loop).
///
/// When the animation has root movement data, the movement curve difference between
/// the previous and the current progress is returned, so the entity travels exactly
/// as much as the animation moves its base pos bone.
#[allow(clippy::cast_precision_loss)]
pub(crate) fn advance_animation(data: &AnimationState, delta: f32) -> Option<FrameUpdate> {
    if data.fps <= 0.0 || data.frames_num < 1 {
        return None;
    }
    let frames_num = data.frames_num;

    // Single frame poses hold their frame: they never finish on their own and
    // are left only via an interrupting switch.
    if frames_num == 1 {
        return Some(FrameUpdate {
            start_frame: 0,
            end_frame: 0,
            mul_1000: 0,
            time_us: 0,
            delta_movement: 0.0,
            finished: true,
            old_progress: 0.0,
            new_progress: 0.0,
        });
    }

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
        old_progress: old_time * data.fps,
        new_progress: progress,
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

/// Applies animations to entities. Animations play purely on time; when an
/// animation moves its base pos bone, the entity itself is moved by that root
/// motion if its controller enables it.
/// Queued `next` animations are switched to when the current one ends or may
/// be interrupted.
/// For entities marked as players it emits [`AnimationEnds`] when entering
/// the `last_frame-2` window and [`AnimationTrigger`] when crossing a model
/// script trigger frame, so the sequencer can schedule follow up animations.
#[allow(clippy::cast_precision_loss)]
#[allow(clippy::type_complexity)]
pub(crate) fn update_animations(
    time: Res<Time>,
    animations: Res<Assets<AnimationData>>,
    triggers: Option<Res<AnimationTriggers>>,
    mut ends: MessageWriter<AnimationEnds>,
    mut animation_triggers: MessageWriter<AnimationTrigger>,
    mut query: Query<
        (
            Entity,
            &GlobalTransform,
            Option<&Aabb>,
            Option<&ChildOf>,
            Option<&PlayerCharacter>,
            &mut AnimationState,
            Option<&mut AnimationController>,
        ),
        With<DistanceNear>,
    >,
    q_vel: Query<&LinearVelocity>,
    accelerating: Query<(), With<AcceleratingRootMotion>>,
    mut transforms: Query<&mut Transform>,
    mut joints: Query<&mut BoneAnimationData>,
    frustum: Option<Single<&Frustum, With<Camera3d>>>,
) {
    let delta = time.delta_secs();
    let mut switches: Vec<(Entity, Arc<AnimationData>)> = Vec::new();

    for (entity, global_transform, aabb, parent, player, mut data, controller) in &mut query {
        // Without a main camera (e.g. in tests) fall back to always updating.
        let needs_update = match (&frustum, aabb) {
            (Some(frustum), Some(aabb)) => {
                frustum.intersects_obb(aabb, &global_transform.affine(), true, true)
            }
            (Some(frustum), None) => frustum.intersects_sphere(
                &Sphere {
                    center: global_transform.translation().into(),
                    radius: 1.0,
                },
                true,
            ),
            (None, _) => true,
        };
        // TODO better way to know which entities are visible and which not
        // - check if visible from renderer visibility data
        // - Mark visible entities with component so we could iterate only on visible Entities
        data.frame_state
            .needs_update
            .store(needs_update, Ordering::Release);

        if !data.frame_state.needs_update.load(Ordering::Acquire) {
            continue;
        }
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

        let Some(mut controller) = controller else {
            println!("no controller");
            continue;
        };

        // Root motion: the animation drives the position of the entity (the
        // physics body owns the model as a child, so move that one and let
        // move-and-slide react to the new position).
        if controller.root_motion() && update.delta_movement != 0.0 {
            let target = match parent {
                Some(parent) if q_vel.contains(parent.parent()) => parent.parent(),
                _ => entity,
            };
            let speed = if accelerating.contains(target) {
                ACCELERATION_MULTIPLIER as f32
            } else {
                1.0
            };
            if let Ok(mut transform) = transforms.get_mut(target) {
                transform.translation +=
                    global_transform.rotation() * Vec3::Z * update.delta_movement * speed;
            }
        }

        // The end window drives the animation switch scheduling of the
        // sequencer and the hit reaction resume; consumers filter by `who`.
        let frames_num = data.frames_num as f32;
        // The receiver has two frames to schedule the next animation
        let end_window = (frames_num as usize).saturating_sub(2) as f32;
        // Single frame poses hold their frame (progress stays 0, they
        // never cross the end window): they count as ended, so a
        // sequencer can leave them as soon as the desired animation
        // differs from the held pose.
        let crossed_end = data.frames_num == 1
            || update.finished
            || (update.old_progress < end_window && update.new_progress >= end_window);
        if crossed_end {
            ends.write(AnimationEnds {
                who: entity,
                what_animation: controller.current().clone(),
            });
        }

        // Animation trigger events are emitted only for players for now; the
        // sequencer consuming them is player-only.
        if player.is_some()
            && let Some(triggers) = triggers.as_ref()
            && let Some(path) = controller.current().path()
            && let Some(animation) = animation_name_from_path(path.path())
        {
            for trigger in crossed_triggers(
                triggers.triggers(animation),
                update.old_progress,
                update.new_progress,
                update.finished,
            ) {
                animation_triggers.write(AnimationTrigger {
                    who: entity,
                    trigger_name: trigger.name.clone(),
                });
            }
        }

        // // Uncomment to debug animations
        // if update.finished && controller.next.is_none() {
        //     info!("Loop animation({:?})", controller.current.path());
        // }

        // Switch when the current animation ended. Interruptible animations are
        // switched immediately, otherwise e.g. the 2s idle loop would delay
        // movement start.
        if (update.finished || controller.interrupt_current())
            && controller
                .next()
                .is_some_and(|next| next != controller.current())
            && let Some(next) = controller.take_next()
            && let Some(next_data) = animations.get(&next)
        {
            let next_data = Arc::new(next_data.clone());
            let current_path = controller.current().path();
            let next_path = Handle::path(&next);
            if next_data.frames_num == 0 {
                warn!(
                    "Animation switch to {next_path:?} on entity({entity}) skipped: not playable (frames {})",
                    next_data.frames_num,
                );
                continue;
            }
            debug!("Animation switch on entity({entity}): {current_path:?} -> {next_path:?}",);
            data.fps = next_data.fps;
            data.frames_num = next_data.frames_num as usize;
            data.movement = next_data.root_movement(data.base_pos_node_index);
            // New animation starts from its first frame
            data.time_us.store(0, Ordering::Release);
            data.frame_state.start_frame.store(0, Ordering::Release);
            // Single frame poses have no second frame to interpolate to
            data.frame_state.end_frame.store(
                if next_data.frames_num > 1 { 1 } else { 0 },
                Ordering::Release,
            );
            data.frame_state.mul_1000.store(0, Ordering::Release);
            controller.switch_to(next);
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
        if !shared.needs_update.load(Ordering::Acquire) {
            return;
        }
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
