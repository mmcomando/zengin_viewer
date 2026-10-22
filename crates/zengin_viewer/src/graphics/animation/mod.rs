mod auto_anim_sequencer;
mod components;
mod controller;
mod data;
mod locomotion;
mod npc;
mod plugin;

#[cfg(test)]
mod tests;

pub use components::{AnimatedJoint, AnimationEnds, AnimationState, BoneAnimationData, FrameState};
pub use controller::AnimationController;
pub use data::{AnimationData, AnimationSample};
pub use npc::NpcAnimations;
pub use plugin::SkeletalAnimations;
