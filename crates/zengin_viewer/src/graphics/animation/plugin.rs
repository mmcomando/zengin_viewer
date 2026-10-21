use bevy::prelude::*;

use crate::gameplay::character::move_and_slide;

use super::ScriptedAnimations;
use super::auto_anim_sequencer::AutoAnimSequencerPlugin;
use super::components::{AnimationEnds, AnimationTrigger};
use super::locomotion::{compute_animations, update_animations};

#[derive(Default)]
pub struct SkeletalAnimations;

impl Plugin for SkeletalAnimations {
    fn build(&self, app: &mut App) {
        app.add_message::<AnimationEnds>();
        app.add_message::<AnimationTrigger>();
        // Disabled in favour of the graph driven auto anim sequencer
        app.add_plugins(AutoAnimSequencerPlugin);

        // Root movement has to be applied before the character controller,
        // so move-and-slide starts from the new position and can react to it.
        // Scripted animation sequences run before the animation advance,
        // see [`ScriptedAnimations`].
        app.configure_sets(Update, ScriptedAnimations.before(update_animations));
        app.add_systems(
            Update,
            (update_animations.before(move_and_slide), compute_animations).chain(),
        );
    }
}
