mod animation_graph_nodes;
mod animation_sounds;
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
use crate::zengin::NpcMobUse;
use components::AnimationTrigger;
use locomotion::{compute_animations, update_animations};

pub(crate) use animation_graph_nodes::{AnimationGraph, AnimationIndex};
pub(crate) use animation_sounds::play_animation_sounds;
pub(crate) use animation_triggers::{
    AnimationTriggers, AnimationTriggersPlugin, animation_name_from_path,
};
pub(crate) use auto_anim_sequencer::{
    AutoAnimSequencer, AutoAnimSequencerSet, DesiredAnimation, MeleeAttackKind,
    SetDesiredAnimation, is_melee_attack_animation,
};
pub(crate) use components::{
    AnimatedJoint, AnimationEnds, AnimationSound, AnimationState, BoneAnimationData, FrameState,
    NpcAnimations,
};
pub(crate) use controller::AnimationController;
pub(crate) use data::{AnimationData, AnimationSample};
pub(crate) use hit_reaction::{NpcHitReaction, npc_hit_reaction_end, play_hit_reaction};

/// Root plugin of the animation module.
#[derive(Default)]
pub struct SkeletalAnimations;

fn update_mob_pose_offset(skeletons: Query<(&AnimationState, Option<&NpcMobUse>)>) {
    for (state, mob_use) in &skeletons {
        let offset = mob_use.map_or(Vec3::ZERO, |mob_use| mob_use.pose_offset);
        let cancel_root = mob_use.is_some_and(|mob_use| mob_use.cancel_root_translation);
        state
            .frame_state
            .cancel_root_translation
            .store(cancel_root, std::sync::atomic::Ordering::Release);
        *state
            .frame_state
            .pose_offset
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = offset;
    }
}

impl Plugin for SkeletalAnimations {
    fn build(&self, app: &mut App) {
        app.add_message::<AnimationEnds>();
        app.add_message::<AnimationTrigger>();
        app.add_message::<AnimationSound>();
        app.add_plugins(auto_anim_sequencer::AutoAnimSequencerPlugin);

        // Root movement has to be applied before the character controller,
        // so move-and-slide starts from the new position and can react to it.
        app.add_systems(
            Update,
            (
                update_animations.before(move_and_slide),
                update_mob_pose_offset,
                compute_animations,
            )
                .chain()
                .run_if(in_state(AppState::InGame)),
        );
        app.add_systems(Update, play_animation_sounds.after(update_animations));
    }
}
