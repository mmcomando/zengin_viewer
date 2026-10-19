mod components;
mod data;
mod locomotion;
mod npc;
mod plugin;

#[cfg(test)]
mod tests;

use bevy::prelude::*;

pub use components::{
    AnimatedJoint, AnimationController, AnimationState, BoneAnimationData, FrameState,
    LocomotionSequencer,
};
pub use data::{AnimationData, AnimationSample};
pub use npc::{MovementAnimations, NpcAnimations};
pub use plugin::SkeletalAnimations;

/// System set for scripted animation sequences (e.g. the bow attack). Systems
/// in this set run after locomotion picked its animation and before the
/// animations advance, so a queued scripted animation is not overwritten by
/// locomotion and takes effect in the same frame.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ScriptedAnimations;
