use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::gameplay::character::move_and_slide;
use crate::gameplay::distance::DistanceNear;
use crate::gui_debug::toggle_visibility::show_gizmos;
use avian3d::prelude::LinearVelocity;
use bevy::prelude::*;

#[derive(Default)]
pub struct SkeletalAnimations;

/// Animation config of an npc
#[derive(Debug, Default, Component)]
pub struct NpcAnimations {
    /// Animation played when [`Self::movement_animations`] is not set,
    /// otherwise used as fallback when the entity has no movement animations
    pub animation: Option<String>,
    /// When set, the animation controller switches between those animations
    /// based on entity movement
    pub movement_animations: Option<MovementAnimations>,
}

/// Animations driving entity movement. Transitions are played once, the others loop.
#[derive(Debug, Clone)]
pub struct MovementAnimations {
    /// Played while the entity does not move
    pub idle: String,
    /// Played once when the entity starts moving
    pub move_start: Option<String>,
    /// Played while the entity moves
    pub move_loop: String,
    /// Played once when the entity stops moving
    pub move_stop: Option<String>,
}

impl Plugin for SkeletalAnimations {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, draw_bones.run_if(show_gizmos));

        // Root movement has to be applied before the character controller,
        // so move-and-slide starts from the new position and can react to it.
        app.add_systems(
            Update,
            (
                update_locomotion,
                update_animations.before(move_and_slide),
                compute_animations,
            )
                .chain(),
        );
    }
}

/// Position and rotation of a bone in a single animation frame.
#[derive(Debug, Clone, Copy)]
pub struct AnimationSample {
    pub position: Vec3,
    pub rotation: Quat,
}

impl Default for AnimationSample {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
        }
    }
}

/// Skeletal animation asset loaded from a zengin `.MAN` file.
#[derive(TypePath, Asset, Debug, Clone, Default)]
pub struct AnimationData {
    samples: Vec<AnimationSample>,
    bone_indices: Vec<u32>,
    pub frames_num: u32,
    pub fps: f32,
}

impl AnimationData {
    /// Creates animation data from bone samples ordered frame by frame.
    pub fn new(
        samples: Vec<AnimationSample>,
        bone_indices: Vec<u32>,
        frames_num: u32,
        fps: f32,
    ) -> Self {
        Self {
            samples,
            bone_indices,
            frames_num,
            fps,
        }
    }

    pub fn get_bone_sample(&self, frame_index: usize, bone_index: usize) -> AnimationSample {
        assert!(self.frames_num > 1);
        let bones_num = self.bone_indices.len();
        if let Some(sample) = self.samples.get(bones_num * frame_index + bone_index) {
            return *sample;
        }
        if !self.samples.is_empty() {
            return self.samples[0];
        }
        println!(
            "Not found sample for frame_index({frame_index}), bone_index({bone_index}), all samples({}), bones_num({}), ii({})",
            self.samples.len(),
            bones_num,
            bones_num * frame_index + bone_index
        );
        return AnimationSample::default();
    }

    pub fn get_index_for_bone(&self, bone_index: usize) -> Option<usize> {
        self.bone_indices
            .iter()
            .position(|el| *el == bone_index as u32)
    }

    pub fn get_movement(&self, bone_index: usize) -> Vec<f32> {
        let local_bone_index = self.get_index_for_bone(bone_index).unwrap();

        let mut movement = vec![];

        let start = self.get_bone_sample(0, local_bone_index).position.z;

        for frame_index in 0..self.frames_num as usize {
            let position = self.get_bone_sample(frame_index, local_bone_index).position;
            movement.push(position.z - start);
        }
        return movement;
    }

    #[allow(clippy::cast_precision_loss)]
    pub fn compute_average_min_max_position_for_bone(
        &self,
        bone_index: usize,
    ) -> Option<(Vec3, Vec3, Vec3)> {
        let local_bone_index = self.get_index_for_bone(bone_index)?;
        let bones_num = self.bone_indices.len();
        if bones_num == 0 || self.frames_num == 0 {
            return None;
        }

        let first_sample = self.get_bone_sample(0, local_bone_index);
        let mut position_sum = Vec3::ZERO;
        let mut min_position = first_sample.position;
        let mut max_position = first_sample.position;

        for frame_index in 0..self.frames_num as usize {
            let position = self.get_bone_sample(frame_index, local_bone_index).position;
            position_sum += position;
            min_position = min_position.min(position);
            max_position = max_position.max(position);
        }

        let average_position = position_sum / self.frames_num as f32;
        Some((average_position, min_position, max_position))
    }

    pub fn bone_has_movement(&self, bone_index: usize) -> bool {
        let Some((_average_position, min_position, max_position)) =
            self.compute_average_min_max_position_for_bone(bone_index)
        else {
            return false;
        };

        if max_position.z - min_position.z < 0.4 {
            return false;
        }

        return true;
    }

    /// Root movement of the base pos bone, if the animation moves it
    pub fn root_movement(&self, base_pos_node_index: usize) -> Option<Vec<f32>> {
        if self.bone_has_movement(base_pos_node_index) {
            Some(self.get_movement(base_pos_node_index))
        } else {
            None
        }
    }
}

#[derive(Component)]
pub struct AnimatedJoint;

/// Data a joint needs to sample the currently played animation.
/// Rewritten whenever the skeleton switches to another animation.
#[derive(Component, Debug)]
pub struct BoneAnimationData {
    /// Skeleton entity owning this joint
    pub skeleton: Entity,
    /// Node index in the model hierarchy, shared by all animations of the model
    pub node_index: usize,
    pub is_base_pos_bone: bool,
    /// Local bone index in the current animation.
    /// `None` when the animation does not animate this node (joint holds its last pose).
    pub bone_index: Option<usize>,
    pub animation_data: Arc<AnimationData>,
    pub frame_state: Arc<FrameState>,
}

/// Interpolation state shared between the skeleton and all of its joints.
#[derive(Debug)]
pub struct FrameState {
    pub start_frame: AtomicU32,
    pub end_frame: AtomicU32,
    pub mul_1000: AtomicU32,
}

/// Playback state of the animation currently played by a skeleton.
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
    /// Node index of the base pos bone, used to extract root movement of switched animations
    pub base_pos_node_index: usize,
}

/// Playback control of a skeleton.
///
/// When the current animation ends, [`AnimationController::next`] is played;
/// if it is `None`, the current animation loops. [`AnimationController::interrupt_current`]
/// allows the queued `next` to replace a still playing animation immediately —
/// otherwise one-shot animations have to play to their end first.
///
/// What to play next is decided by other systems: moving entities get their
/// controller driven by a [`LocomotionSequencer`].
#[derive(Component, Debug)]
pub struct AnimationController {
    /// Currently played animation
    pub current: Handle<AnimationData>,
    /// Animation to play when the current one ends; `None` loops the current one
    pub next: Option<Handle<AnimationData>>,
    /// Whether the queued `next` may replace `current` immediately
    pub interrupt_current: bool,
}

/// Movement animation set driving the [`AnimationController`] of moving entities.
///
/// A system watches entity movement and writes `next` and `interrupt_current`
/// of the controller. Entities without movement animations don't have this
/// component and simply loop their controller's `current` animation.
#[derive(Component, Debug)]
pub struct LocomotionSequencer {
    /// Played while the entity does not move
    pub idle: Handle<AnimationData>,
    /// Played once when the entity starts moving
    pub move_start: Option<Handle<AnimationData>>,
    /// Played while the entity moves
    pub move_loop: Handle<AnimationData>,
    /// Played once when the entity stops moving
    pub move_stop: Option<Handle<AnimationData>>,
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
/// Horizontal velocity above which an entity counts as moving
const MOVEMENT_THRESHOLD_SQUARED: f32 = 0.5 * 0.5;

/// Result of advancing one animation by one time step.
struct FrameUpdate {
    start_frame: usize,
    end_frame: usize,
    mul_1000: u32,
    time_us: u32,
    /// How much root movement the entity should be moved by this step.
    #[allow(unused)]
    delta_movement: f32,
    /// Whether the animation completed at least one loop this step
    finished: bool,
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
enum AnimationKind {
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
fn animation_kind(
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
fn desired_next(
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
fn update_locomotion(
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
fn update_animations(
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
fn compute_animations(mut query: Query<(&mut Transform, &BoneAnimationData)>) {
    for (mut transform, data) in &mut query {
        let Some(bone_index) = data.bone_index else {
            continue;
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

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
            base_pos_node_index: 0,
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
        assert!(!update.finished);
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
        assert!(!update.finished);
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
        assert!(update.finished);
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

    /// Distinct handles backed by a real asset store
    fn test_sequencer() -> (Assets<AnimationData>, LocomotionSequencer) {
        let mut assets = Assets::<AnimationData>::default();
        let idle = assets.add(AnimationData::default());
        let move_start = assets.add(AnimationData::default());
        let move_loop = assets.add(AnimationData::default());
        let move_stop = assets.add(AnimationData::default());
        (
            assets,
            LocomotionSequencer {
                idle,
                move_start: Some(move_start),
                move_loop,
                move_stop: Some(move_stop),
            },
        )
    }

    #[test]
    fn movement_starts_with_transition_into_loop() {
        let (_assets, sequencer) = test_sequencer();
        let move_start = sequencer.move_start.clone().unwrap();

        // Standing still keeps looping the idle animation
        assert_eq!(
            animation_kind(&sequencer, &sequencer.idle),
            Some(AnimationKind::Idle)
        );
        assert_eq!(
            desired_next(&sequencer, AnimationKind::Idle, false),
            (None, true)
        );

        // Starting to move queues the start transition, replacing the idle
        // loop immediately
        assert_eq!(
            desired_next(&sequencer, AnimationKind::Idle, true),
            (Some(move_start.clone()), true)
        );

        // The start transition leads into the movement loop, playing out first
        assert_eq!(
            animation_kind(&sequencer, &move_start),
            Some(AnimationKind::MoveStart)
        );
        assert_eq!(
            desired_next(&sequencer, AnimationKind::MoveStart, true),
            (Some(sequencer.move_loop.clone()), false)
        );
    }

    #[test]
    fn stopping_movement_transitions_back_to_idle() {
        let (_assets, sequencer) = test_sequencer();
        let move_stop = sequencer.move_stop.clone().unwrap();

        // Running keeps the movement loop
        assert_eq!(
            animation_kind(&sequencer, &sequencer.move_loop),
            Some(AnimationKind::MoveLoop)
        );
        assert_eq!(
            desired_next(&sequencer, AnimationKind::MoveLoop, true),
            (None, true)
        );

        // Stopping queues the stop transition, replacing the run loop
        // immediately
        assert_eq!(
            desired_next(&sequencer, AnimationKind::MoveLoop, false),
            (Some(move_stop.clone()), true)
        );

        // The stop transition leads back to idle, playing out first
        assert_eq!(
            animation_kind(&sequencer, &move_stop),
            Some(AnimationKind::MoveStop)
        );
        assert_eq!(
            desired_next(&sequencer, AnimationKind::MoveStop, false),
            (Some(sequencer.idle.clone()), false)
        );
        assert_eq!(
            desired_next(&sequencer, AnimationKind::MoveStop, true),
            (Some(sequencer.move_start.clone().unwrap()), false)
        );
    }

    #[test]
    fn missing_transitions_fall_back_to_loops() {
        let (_assets, mut sequencer) = test_sequencer();
        sequencer.move_start = None;
        sequencer.move_stop = None;

        // Start of movement goes straight into the movement loop
        assert_eq!(
            desired_next(&sequencer, AnimationKind::Idle, true),
            (Some(sequencer.move_loop.clone()), true)
        );
        // Stop of movement goes straight back to idle
        assert_eq!(
            desired_next(&sequencer, AnimationKind::MoveLoop, false),
            (Some(sequencer.idle.clone()), true)
        );
    }

    #[test]
    fn foreign_animations_are_left_alone() {
        let (mut assets, sequencer) = test_sequencer();
        let scripted = assets.add(AnimationData::default());

        // Animations the sequencer does not own (e.g. scripted actions) have
        // no kind; update_locomotion leaves their controller alone
        assert_eq!(animation_kind(&sequencer, &scripted), None);
    }

    /// Builds a world with a single near skeleton for `update_animations`.
    /// Time does not advance; the animation progress is pre-loaded via the
    /// state's `time_us`.
    fn run_update_animations(
        assets: Assets<AnimationData>,
        state: AnimationState,
        controller: AnimationController,
    ) -> (World, Entity) {
        let mut world = World::new();
        world.insert_resource(assets);
        // `update_animations` reads the generic `Time`, which TimePlugin
        // normally inserts alongside the specialized ones
        world.insert_resource(Time::<()>::default());
        let model = world
            .spawn((
                DistanceNear::default(),
                state,
                controller,
                GlobalTransform::default(),
            ))
            .id();
        (world, model)
    }

    #[test]
    fn queued_next_replaces_finished_animation() {
        let mut assets = Assets::<AnimationData>::default();
        let first = assets.add(AnimationData::default());
        let second = assets.add(AnimationData::default());

        // 2 s into the 0.8 s loop: the animation finished this step
        let state = runl_state(Some(runl_movement()));
        state.time_us.store(2_000_000, Ordering::Release);
        let (mut world, model) = run_update_animations(
            assets,
            state,
            AnimationController {
                current: first,
                next: Some(second.clone()),
                interrupt_current: false,
            },
        );
        world.run_system_once(update_animations).unwrap();

        let controller = world.get::<AnimationController>(model).unwrap();
        assert_eq!(controller.current, second);
        assert_eq!(controller.next, None);
    }

    #[test]
    fn interrupt_current_replaces_playing_animation() {
        let mut assets = Assets::<AnimationData>::default();
        let first = assets.add(AnimationData::default());
        let second = assets.add(AnimationData::default());

        // The animation just started, but the queued next may interrupt it
        let (mut world, model) = run_update_animations(
            assets,
            runl_state(Some(runl_movement())),
            AnimationController {
                current: first,
                next: Some(second.clone()),
                interrupt_current: true,
            },
        );
        world.run_system_once(update_animations).unwrap();

        let controller = world.get::<AnimationController>(model).unwrap();
        assert_eq!(controller.current, second);
        assert_eq!(controller.next, None);
    }

    #[test]
    fn queued_next_waits_for_current_to_end() {
        let mut assets = Assets::<AnimationData>::default();
        let first = assets.add(AnimationData::default());
        let second = assets.add(AnimationData::default());

        // The animation just started and may not be interrupted
        let (mut world, model) = run_update_animations(
            assets,
            runl_state(Some(runl_movement())),
            AnimationController {
                current: first.clone(),
                next: Some(second.clone()),
                interrupt_current: false,
            },
        );
        world.run_system_once(update_animations).unwrap();

        let controller = world.get::<AnimationController>(model).unwrap();
        assert_eq!(controller.current, first);
        assert_eq!(controller.next, Some(second));
    }
}
