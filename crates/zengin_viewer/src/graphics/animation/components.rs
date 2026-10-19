use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32};

use bevy::prelude::*;

use super::data::AnimationData;

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
    /// Set by `update_animations` depending on whether the skeleton is inside
    /// the main camera frustum. Joints skip bone computation when it is `false`.
    /// Starts as `true` so the first frame is processed before the check runs.
    pub needs_update: AtomicBool,
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
    /// Whether the root movement of the current animation moves the entity
    /// itself (Gothic-style root motion: the animation drives the position,
    /// the character controller only handles gravity and collisions).
    pub should_move_entity: bool,
}

/// Sent when an animation enters its `last_frame-2` window — the receiver has
/// two frames to schedule the next animation before the current one ends.
/// Fires on every loop wrap of looping animations.
// Consumers come with the sequencer phase.
#[allow(dead_code)]
#[derive(Message, Debug)]
pub struct AnimationEnds {
    /// Skeleton entity playing the animation
    pub who: Entity,
    pub what_animation: Handle<AnimationData>,
}

/// Sent when the currently played animation crosses a trigger frame declared
/// in the model script (e.g. the arrow release event).
// Consumers come with the sequencer phase.
#[allow(dead_code)]
#[derive(Message, Debug)]
pub struct AnimationTrigger {
    /// Skeleton entity playing the animation
    pub who: Entity,
    pub trigger_name: String,
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
