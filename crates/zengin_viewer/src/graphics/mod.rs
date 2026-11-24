mod animation;
pub(crate) use crate::graphics::animation::{
    NpcHitReaction, npc_hit_reaction_end, play_hit_reaction,
};
mod lighting;
mod skybox;

pub(crate) use crate::graphics::animation::{
    AnimatedJoint, AnimationController, AnimationData, AnimationEnds, AnimationGraph,
    AnimationIndex, AnimationSample, AnimationState, AnimationTriggers, AnimationTriggersPlugin,
    AutoAnimSequencer, AutoAnimSequencerSet, BoneAnimationData, DesiredAnimation, FrameState,
    MeleeAttackKind, NpcAnimations, PlayerMeleeCombo, SetDesiredAnimation, SkeletalAnimations,
    animation_name_from_path, is_melee_attack_animation,
};
pub(crate) use crate::graphics::lighting::LightingPlugin;
pub(crate) use crate::graphics::skybox::SkyBoxPlugin;

use bevy::prelude::*;

/// Runtime-toggleable rendering quality options, edited in the debug UI.
#[derive(Resource)]
pub struct RenderOptions {
    /// Enable sun shadow maps.
    pub sun_shadow_maps: bool,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            sun_shadow_maps: true,
        }
    }
}

pub struct RenderOptionsPlugin;

impl Plugin for RenderOptionsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(RenderOptions::default());
    }
}
