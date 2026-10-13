mod components;
mod data;
mod locomotion;
mod npc;
mod plugin;

#[cfg(test)]
mod tests;

pub use components::{
    AnimatedJoint, AnimationController, AnimationState, BoneAnimationData, FrameState,
    LocomotionSequencer,
};
pub use data::{AnimationData, AnimationSample};
pub use npc::{MovementAnimations, NpcAnimations};
pub use plugin::SkeletalAnimations;
