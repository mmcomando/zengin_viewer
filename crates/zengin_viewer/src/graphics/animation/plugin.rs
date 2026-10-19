use bevy::prelude::*;

use crate::gameplay::character::move_and_slide;
use crate::gui_debug::toggle_visibility::show_gizmos;

use super::ScriptedAnimations;
use super::components::AnimatedJoint;
use super::locomotion::{compute_animations, update_animations, update_locomotion};

#[derive(Default)]
pub struct SkeletalAnimations;

impl Plugin for SkeletalAnimations {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, draw_bones.run_if(show_gizmos));

        // Root movement has to be applied before the character controller,
        // so move-and-slide starts from the new position and can react to it.
        // Scripted animation sequences run between locomotion and the
        // animation advance, see [`ScriptedAnimations`].
        app.configure_sets(
            Update,
            ScriptedAnimations
                .after(update_locomotion)
                .before(update_animations),
        );
        app.add_systems(
            Update,
            (
                update_locomotion,
                update_animations.before(move_and_slide),
                compute_animations,
            )
                .chain(),
        );
    }
}

fn draw_bones(
    mut gizmos: Gizmos,

    bones: Query<(Entity, &GlobalTransform, Option<&ChildOf>), With<AnimatedJoint>>,
    transforms: Query<&GlobalTransform>,
) {
    for (_bone_entity, bone_transform, parent) in bones.iter() {
        if let Some(parent) = parent
            && let Ok(parent_transform) = transforms.get(parent.0)
        {
            // Draw a line from parent bone to current bone
            gizmos.line(
                parent_transform.translation(),
                bone_transform.translation(),
                Color::srgb(0.0, 1.0, 0.0), // Green bones
            );

            // Optional: Draw the bone itself (a small sphere)
            // gizmos.sphere(
            //     bone_transform.translation(),
            //     // Quat::IDENTITY,
            //     0.02,
            //     Color::srgb(1.0, 0.0, 0.0), // Red joints
            // );
        }
    }
}
