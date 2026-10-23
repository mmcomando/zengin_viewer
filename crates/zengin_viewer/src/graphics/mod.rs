mod animation;
mod disco_lamp;
pub(crate) use crate::graphics::animation::{
    NpcHitReaction, npc_hit_reaction_end, play_hit_reaction,
};
mod lighting;
mod skybox;

pub(crate) use crate::graphics::animation::{
    AnimatedJoint, AnimationController, AnimationData, AnimationGraph, AnimationIndex,
    AnimationSample, AnimationState, AnimationTriggersPlugin, BoneAnimationData, FrameState,
    NpcAnimations, SkeletalAnimations, animation_name_from_path,
};
pub(crate) use crate::graphics::disco_lamp::{DiscoLamp, DrawGizmo};
pub(crate) use crate::graphics::lighting::LightingPlugin;
pub(crate) use crate::graphics::skybox::SkyBoxPlugin;

use bevy::prelude::*;

/// Runtime-toggleable rendering quality options, edited in the debug UI.
#[derive(Resource)]
pub struct RenderOptions {
    /// Enable sun shadow maps.
    pub sun_shadow_maps: bool,
    /// Enables shadow maps for the disco lamp point lights (very heavy).
    pub disco_shadow_maps: bool,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            sun_shadow_maps: true,
            disco_shadow_maps: false,
        }
    }
}

pub struct RenderOptionsPlugin;

impl Plugin for RenderOptionsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(RenderOptions::default());
    }
}
