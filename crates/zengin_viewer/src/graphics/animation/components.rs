use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32};

use bevy::prelude::*;

use super::data::AnimationData;

#[derive(Component)]
pub struct AnimatedJoint;

/// Animation config of an npc
#[derive(Debug, Default, Component)]
pub struct NpcAnimations {
    /// Animation the entity loops; `None` falls back to the stand animation
    /// of the model scheme
    pub animation: Option<String>,
}

/// Data a joint needs to sample the currently played animation.
/// Rewritten whenever the skeleton switches to another animation.
#[derive(Component, Debug)]
pub struct BoneAnimationData {
    /// Skeleton entity owning this joint
    pub skeleton: Entity,
    /// Node index in the model hierarchy, shared by all animations of the model.
    /// Not the same as `bone_index`: a `.MAN` animates only a subset of the
    /// hierarchy nodes (its `node_indices` can be e.g. `[7]`), so this index
    /// is the key used to find the node in an animation's node list.
    pub node_index: usize,
    pub is_base_pos_bone: bool,
    /// Index of this node inside the current animation's `node_indices` list,
    /// i.e. the offset into its per-frame sample array.
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

/// Sent when an animation enters its `last_frame-2` window — the receiver has
/// two frames to schedule the next animation before the current one ends.
/// Fires on every loop wrap of looping animations.
#[derive(Message, Debug, Clone)]
pub struct AnimationEnds {
    /// Skeleton entity playing the animation
    pub who: Entity,
    pub what_animation: Handle<AnimationData>,
}

/// Sent when the currently played animation crosses a trigger frame declared
/// in the model script (e.g. the arrow release event).
#[derive(Message, Debug, Clone)]
pub struct AnimationTrigger {
    /// Skeleton entity playing the animation
    // No consumer yet: the sequencer phase reading triggers was removed;
    // emission and the `HUMANS.MDS` parsing behind it are kept for it.
    #[allow(dead_code)]
    pub who: Entity,
    #[allow(dead_code)]
    pub trigger_name: String,
}
