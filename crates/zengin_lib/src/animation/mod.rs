pub mod animation_graph_nodes;
pub mod animation_sounds;
pub mod animation_triggers;
pub mod auto_anim_sequencer;
pub mod components;
pub mod controller;
pub mod corpse;
pub mod data;
pub mod hit_reaction;
pub mod locomotion;
pub mod player_melee_combo;
pub mod send_desired;

#[cfg(test)]
pub mod tests;

use bevy::prelude::*;

use crate::AppState;
use crate::NpcMobUse;
use crate::gameplay::character::move_and_slide;
use components::AnimationTrigger;
use locomotion::{compute_animations, update_animations};

pub use animation_graph_nodes::{AnimationGraph, AnimationIndex};
pub use animation_sounds::play_animation_sounds;
pub use animation_triggers::{
    AnimationTriggers, AnimationTriggersPlugin, animation_name_from_path,
};
pub use auto_anim_sequencer::{
    AutoAnimSequencer, AutoAnimSequencerSet, DesiredAnimation, MeleeAttackKind,
    SetDesiredAnimation, is_melee_attack_animation,
};
pub use components::{
    AnimatedJoint, AnimationEnds, AnimationSound, AnimationState, BoneAnimationData, FrameState,
    NpcAnimations,
};
pub use controller::AnimationController;
pub use corpse::apply_corpse_animation;
pub use data::{AnimationData, AnimationSample};
pub use hit_reaction::{NpcHitReaction, npc_hit_reaction_end, play_hit_reaction};
pub use player_melee_combo::PlayerMeleeCombo;

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
        // Runs after the sequencer systems so the corpse death animation is
        // queued last and is not replaced by a stale desired animation.
        app.add_systems(
            Update,
            apply_corpse_animation
                .after(AutoAnimSequencerSet)
                .run_if(in_state(AppState::InGame)),
        );
    }
}
