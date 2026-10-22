mod auto_anim_sequencer;
mod components;
mod controller;
mod data;
mod locomotion;
mod npc;
mod plugin;

#[cfg(test)]
mod tests;

use bevy::prelude::*;

pub use components::{AnimatedJoint, AnimationEnds, AnimationState, BoneAnimationData, FrameState};
pub use controller::AnimationController;
pub use data::{AnimationData, AnimationSample};
pub use npc::NpcAnimations;
pub use plugin::SkeletalAnimations;

/// System set for scripted animation sequences (e.g. the bow attack). Systems
/// in this set run after locomotion picked its animation and before the
/// animations advance, so a queued scripted animation is not overwritten by
/// locomotion and takes effect in the same frame.
/// TODO there is only one place it is used so it changes ordering in any way? maybe it is leftover from old code? remove if not needed
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ScriptedAnimations;
