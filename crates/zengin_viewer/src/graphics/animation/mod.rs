mod animation_graph_nodes;
mod animation_triggers;
mod auto_anim_sequencer;
mod components;
mod controller;
mod data;
mod hit_reaction;
mod locomotion;
mod send_desired;

#[cfg(test)]
mod tests;

use bevy::prelude::*;

use crate::game_state::AppState;
use crate::gameplay::character::move_and_slide;
use components::AnimationTrigger;
use locomotion::{compute_animations, update_animations};

pub(crate) use animation_graph_nodes::{AnimationGraph, AnimationIndex};
pub(crate) use animation_triggers::{AnimationTriggersPlugin, animation_name_from_path};
pub(crate) use components::{
    AnimatedJoint, AnimationEnds, AnimationState, BoneAnimationData, FrameState, NpcAnimations,
};
pub(crate) use controller::AnimationController;
pub(crate) use data::{AnimationData, AnimationSample};
pub(crate) use hit_reaction::{NpcHitReaction, npc_hit_reaction_end, play_hit_reaction};

/// Root plugin of the animation module.
#[derive(Default)]
pub struct SkeletalAnimations;

impl Plugin for SkeletalAnimations {
    fn build(&self, app: &mut App) {
        app.add_message::<AnimationEnds>();
        app.add_message::<AnimationTrigger>();
        app.add_plugins(auto_anim_sequencer::AutoAnimSequencerPlugin);

        // Root movement has to be applied before the character controller,
        // so move-and-slide starts from the new position and can react to it.
        app.add_systems(
            Update,
            (update_animations.before(move_and_slide), compute_animations)
                .chain()
                .run_if(in_state(AppState::InGame)),
        );
    }
}
