use std::sync::Arc;
use std::sync::atomic::Ordering;

use avian3d::prelude::LinearVelocity;
use bevy::{
    camera::primitives::{Aabb, Frustum, Sphere},
    prelude::*,
};

use super::animation_triggers::{
    AnimationTriggers, animation_ident_from_path, crossed_sounds, crossed_triggers,
};
use crate::PlayerCharacter;
use crate::gameplay::character::{
    ACCELERATION_MULTIPLIER, AcceleratingRootMotion, NpcPhysicsActiveMarker,
};
use crate::gameplay::distance::DistanceNear;
use dev_util::perf_debug;

use super::auto_anim_sequencer::AutoAnimSequencer;
use super::components::{
    AnimationEnds, AnimationSound, AnimationState, AnimationTrigger, BoneAnimationData,
};
use super::controller::AnimationController;
use super::data::AnimationData;

pub const ATOMIC_SCALE: f32 = 1000.0;
/// Animation time is stored scaled to integers so it can be shared without locks.
pub const TIME_SCALE: f32 = 1_000_000.0;
/// Skeletons outside the camera frustum animate only while closer than this
/// to the main camera, so nearby characters behind the camera stay in motion.
const ANIMATION_UPDATE_DISTANCE_SQUARED: f32 = 20.0 * 20.0;

/// Result of advancing one animation by one time step.
pub struct FrameUpdate {
    pub start_frame: usize,
    pub end_frame: usize,
    pub mul_1000: u32,
    pub time_us: u32,
    /// Horizontal root movement (local X, Z) the entity should be moved by
    /// this step.
    pub delta_movement: Vec2,
    /// Whether the animation completed at least one loop this step.
    /// Consumers only distinguish "wrapped" from "not wrapped", the loop
    /// count itself is internal to the root movement computation.
    pub finished: bool,

    pub old_progress: f32,
    pub new_progress: f32,
}

/// Advances an animation by `delta` seconds on its natural pace
/// (`frames_num / fps` per loop).
///
/// When the animation has root movement data, the movement curve difference between
/// the previous and the current progress is returned, so the entity travels exactly
/// as much as the animation moves its base pos bone.
#[allow(clippy::cast_precision_loss)]
pub fn advance_animation(data: &AnimationState, delta: f32) -> Option<FrameUpdate> {
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
            delta_movement: Vec2::ZERO,
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

    let delta_movement = data.movement.as_deref().map_or(Vec2::ZERO, |movement| {
        // Movement past the last frame is extrapolated so looping keeps moving in
        // the same direction instead of sliding back to the frame 0 position.
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

/// Root horizontal displacement at animation `progress` (`0..frames_num`).
/// The segment past the last frame interpolates towards the extrapolated
/// full loop displacement (`total`).
fn root_displacement(movement: &[Vec2], total: Vec2, progress: f32) -> Vec2 {
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
/// For entities driven by the animation sequencer (the player and attacking
/// npcs) it emits [`AnimationEnds`] when entering the `last_frame-2` window and
/// [`AnimationTrigger`] when crossing a model script trigger frame, so the
/// sequencer can schedule follow up animations. Every animated skeleton emits
/// [`AnimationSound`] on model script sound frames.
#[allow(clippy::cast_precision_loss)]
#[allow(clippy::type_complexity)]
pub fn update_animations(
    time: Res<Time>,
    animations: Res<Assets<AnimationData>>,
    triggers: Res<AnimationTriggers>,
    mut ends: MessageWriter<AnimationEnds>,
    mut animation_triggers: MessageWriter<AnimationTrigger>,
    mut animation_sounds: MessageWriter<AnimationSound>,
    mut query: Query<
        (
            Entity,
            &GlobalTransform,
            Option<&Aabb>,
            Option<&PlayerCharacter>,
            Option<&AutoAnimSequencer>,
            &mut AnimationState,
            Option<&mut AnimationController>,
        ),
        With<DistanceNear>,
    >,
    accelerating: Query<(), With<AcceleratingRootMotion>>,
    mut velocities: Query<&mut LinearVelocity, With<NpcPhysicsActiveMarker>>,
    mut transforms: Query<&mut Transform>,
    mut joints: Query<&mut BoneAnimationData>,
    camera: Option<Single<(&Frustum, &GlobalTransform), With<Camera3d>>>,
) {
    let perf = perf_debug!();
    let delta = time.delta_secs();
    let mut switches: Vec<(Entity, Arc<AnimationData>)> = Vec::new();

    for (entity, global_transform, aabb, player, sequencer, mut data, controller) in &mut query {
        perf.increment_tag("skeletons", 1);
        let mut velocity = velocities.get_mut(entity).ok();
        // Without a main camera (e.g. in tests) fall back to always updating.
        let needs_update = match (&camera, aabb) {
            (Some(camera), aabb) => {
                let within_near = camera
                    .1
                    .translation()
                    .distance_squared(global_transform.translation())
                    < ANIMATION_UPDATE_DISTANCE_SQUARED;
                let in_frustum = match aabb {
                    Some(aabb) => {
                        camera
                            .0
                            .intersects_obb(aabb, &global_transform.affine(), true, true)
                    }
                    None => camera.0.intersects_sphere(
                        &Sphere {
                            center: global_transform.translation().into(),
                            radius: 1.0,
                        },
                        true,
                    ),
                };
                within_near || in_frustum
            }
            (None, _) => true,
        };
        // TODO better way to know which entities are visible and which not
        // - check if visible from renderer visibility data
        // - Mark visible entities with component so we could iterate only on visible Entities
        data.frame_state
            .needs_update
            .store(needs_update, Ordering::Release);

        if !data.frame_state.needs_update.load(Ordering::Acquire) {
            perf.increment_tag("skipped", 1);
            // A physics npc that stops being animated must not keep the velocity
            // of its last animated step, otherwise it would slide until the
            // animation resumes.
            if let Some(velocity) = velocity.as_deref_mut() {
                velocity.x = 0.0;
                velocity.z = 0.0;
            }
            continue;
        }
        let root_motion = controller
            .as_ref()
            .is_some_and(|controller| controller.root_motion());
        if root_motion {
            perf.increment_tag("root_motion", 1);
        }
        data.frame_state
            .root_motion
            .store(root_motion, Ordering::Release);
        let Some(update) = advance_animation(&data, delta) else {
            if let Some(velocity) = velocity.as_deref_mut() {
                velocity.x = 0.0;
                velocity.z = 0.0;
            }
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

        apply_root_motion(
            entity,
            global_transform,
            &controller,
            &update,
            &accelerating,
            velocity.as_deref_mut(),
            &mut transforms,
            delta,
        );

        // The end window drives the animation switch scheduling of the
        // sequencer and the hit reaction resume; consumers filter by `who`.
        // The window crossing fires ~2 frames before the loop wrap; `finished`
        // is only a fallback for steps that jumped over the whole window (low
        // fps, large delta), otherwise a looping animation would emit two ends
        // per loop (one at the window, one at the wrap).
        let frames_num = data.frames_num as f32;
        // The receiver has two frames to schedule the next animation
        let end_window = (frames_num as usize).saturating_sub(2) as f32;
        // Single frame poses hold their frame (progress stays 0, they
        // never cross the end window): they count as ended, so a
        // sequencer can leave them as soon as the desired animation
        // differs from the held pose.
        let crossed_end = data.frames_num == 1
            || (update.old_progress < end_window && update.new_progress >= end_window)
            || (update.finished && update.old_progress <= end_window);
        if crossed_end {
            ends.write(AnimationEnds {
                who: entity,
                what_animation: controller.current().clone(),
            });
        }

        write_sequencer_triggers(
            player.is_some() || sequencer.is_some(),
            &triggers,
            &controller,
            &update,
            entity,
            &mut animation_triggers,
        );

        write_animation_sounds(
            &triggers,
            &controller,
            &update,
            entity,
            global_transform.translation(),
            &mut animation_sounds,
        );

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
            perf.increment_tag("switches", 1);
            let next_data = Arc::new(next_data.clone());
            switch_animation(
                entity,
                next,
                next_data,
                &mut controller,
                &mut data,
                &mut switches,
            );
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

#[allow(clippy::too_many_arguments)]
fn apply_root_motion(
    entity: Entity,
    global_transform: &GlobalTransform,
    controller: &AnimationController,
    update: &FrameUpdate,
    accelerating: &Query<(), With<AcceleratingRootMotion>>,
    velocity: Option<&mut LinearVelocity>,
    transforms: &mut Query<&mut Transform>,
    delta_secs: f32,
) {
    let speed = if accelerating.contains(entity) {
        ACCELERATION_MULTIPLIER
    } else {
        1.0
    };
    let delta = if controller.root_motion() {
        global_transform.rotation()
            * Vec3::new(update.delta_movement.x, 0.0, update.delta_movement.y)
            * speed
    } else {
        Vec3::ZERO
    };

    // Physics npcs are moved by `move_and_slide`, which integrates the velocity,
    // so the root motion becomes a velocity instead of a direct transform
    // change. Writing it every frame also stops a body whose animation paused.
    if let Some(velocity) = velocity {
        if delta_secs <= 0.0 {
            velocity.x = 0.0;
            velocity.z = 0.0;
            return;
        }
        velocity.x = delta.x / delta_secs;
        velocity.z = delta.z / delta_secs;
        return;
    }

    if delta == Vec3::ZERO {
        return;
    }
    let Ok(mut transform) = transforms.get_mut(entity) else {
        return;
    };
    transform.translation += delta;
}

/// Animation trigger events are emitted for entities driven by the sequencer,
/// which consumes them, e.g. to emit the melee hit on the impact frame.
fn write_sequencer_triggers(
    emit: bool,
    triggers: &AnimationTriggers,
    controller: &AnimationController,
    update: &FrameUpdate,
    entity: Entity,
    animation_triggers: &mut MessageWriter<AnimationTrigger>,
) {
    if !emit {
        return;
    }
    let Some(path) = controller.current().path() else {
        return;
    };
    let Some((scheme, animation)) = animation_ident_from_path(path.path()) else {
        return;
    };
    for trigger in crossed_triggers(
        triggers.triggers(scheme, animation),
        update.old_progress,
        update.new_progress,
        update.finished,
    ) {
        // info!(
        //     "AnimationTrigger: entity={entity:?}, animation={animation}, frame={:.1}, trigger={}",
        //     update.new_progress, trigger.name
        // );
        animation_triggers.write(AnimationTrigger {
            who: entity,
            trigger_name: trigger.name.clone(),
        });
    }
}

fn write_animation_sounds(
    triggers: &AnimationTriggers,
    controller: &AnimationController,
    update: &FrameUpdate,
    entity: Entity,
    position: Vec3,
    animation_sounds: &mut MessageWriter<AnimationSound>,
) {
    let Some(path) = controller.current().path() else {
        return;
    };
    let Some((scheme, animation)) = animation_ident_from_path(path.path()) else {
        return;
    };
    for sound in crossed_sounds(
        triggers.sounds(scheme, animation),
        update.old_progress,
        update.new_progress,
        update.finished,
    ) {
        // info!(
        //     "AnimationSound triggered: entity={entity:?}, animation={animation}, frame={:.1}, sound={}, range={:.0}",
        //     update.new_progress, sound.name, sound.range
        // );
        animation_sounds.write(AnimationSound {
            who: entity,
            sound_name: sound.name.clone(),
            position,
            range: sound.range,
        });
    }
}

/// Applies a queued next animation to the state and schedules the joint
/// resampling. A not playable next animation (`frames_num == 0`) is skipped.
fn switch_animation(
    entity: Entity,
    next: Handle<AnimationData>,
    next_data: Arc<AnimationData>,
    controller: &mut AnimationController,
    data: &mut AnimationState,
    switches: &mut Vec<(Entity, Arc<AnimationData>)>,
) {
    let current_path = controller.current().path();
    let next_path = Handle::path(&next);
    if next_data.frames_num == 0 {
        warn!(
            "Animation switch to {next_path:?} on entity({entity}) skipped: not playable (frames {})",
            next_data.frames_num,
        );
        return;
    }
    debug!("Animation switch on entity({entity}): {current_path:?} -> {next_path:?}",);
    data.fps = next_data.fps;
    data.frames_num = next_data.frames_num as usize;
    data.movement = next_data.root_movement(data.base_pos_node_index);
    // New animation starts from its first frame
    data.time_us.store(0, Ordering::Release);
    data.frame_state.start_frame.store(0, Ordering::Release);
    // Single frame poses have no second frame to interpolate to
    data.frame_state
        .end_frame
        .store(u32::from(next_data.frames_num > 1), Ordering::Release);
    data.frame_state.mul_1000.store(0, Ordering::Release);
    controller.switch_to(next);
    switches.push((entity, next_data));
}

#[allow(clippy::cast_precision_loss)]
#[allow(clippy::type_complexity)]
pub fn compute_animations(mut query: Query<(&mut Transform, &BoneAnimationData)>) {
    let _perf = perf_debug!();
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
            let cancel_root = data
                .frame_state
                .cancel_root_translation
                .load(Ordering::Acquire);
            if data.frame_state.root_motion.load(Ordering::Acquire) || cancel_root {
                // Root-motion animations move the entity with the delta; npc mob
                // poses must stay on their `ZS_POS*` stand point instead of
                // following the pose's horizontal root.
                transform.translation.x = 0.0;
                transform.translation.z = 0.0;
            }
            transform.translation += *data
                .frame_state
                .pose_offset
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
    });
}
