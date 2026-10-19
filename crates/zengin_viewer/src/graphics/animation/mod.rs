mod components;
mod data;
// The sequencer component and systems consuming these types come in a later
// phase; until then the API is intentionally unused.
#[allow(dead_code)]
mod human;
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
pub use human::{HUMAN_ANIMATIONS, HumanAnimationSequence};
pub use npc::{MovementAnimations, NpcAnimations};
pub use plugin::SkeletalAnimations;

/// System set for scripted animation sequences (e.g. the bow attack). Systems
/// in this set run after locomotion picked its animation and before the
/// animations advance, so a queued scripted animation is not overwritten by
/// locomotion and takes effect in the same frame.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ScriptedAnimations;
