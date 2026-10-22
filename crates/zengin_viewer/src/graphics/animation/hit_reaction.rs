//! Npc hit reactions: interrupting the current animation with the
//! `T_GOTHIT` one-shot and restoring it afterwards.

use bevy::prelude::*;

use crate::gameplay::objects::NpcModel;
use crate::zengin::{
    AnimationFiles, MaterialHandles, NpcOverlay, choose_animation_scheme, get_animation_path,
};

use super::{AnimationController, AnimationData, AnimationEnds};

/// Npc currently playing the `T_GOTHIT` hit reaction; carries the animation
/// handle to resume when the reaction one-shot ends.
#[derive(Component)]
pub(crate) struct NpcHitReaction {
    resume: Handle<AnimationData>,
}

/// Interrupts the npc animation with the `T_GOTHIT` hit reaction one-shot,
/// remembering the currently played animation to resume when it ends. A
/// no-op while a reaction is already playing (no flinch stacking).
#[allow(clippy::too_many_arguments)]
pub(crate) fn play_hit_reaction(
    commands: &mut Commands,
    npc: Entity,
    npc_model: &NpcModel,
    overlay: Option<&NpcOverlay>,
    controller: Option<Mut<'_, AnimationController>>,
    reaction: Option<&NpcHitReaction>,
    files: &AnimationFiles,
    handles: &mut MaterialHandles,
    asset_server: &AssetServer,
) {
    if reaction.is_some() {
        return;
    }
    let Some(mut controller) = controller else {
        return;
    };
    let Some(scheme) = npc_model.animation_scheme.as_deref() else {
        return;
    };
    // Base scheme reaction: the weapon overlays have no own `T_GOTHIT`
    let overlay = overlay.and_then(|overlay| overlay.0.as_deref());
    let scheme = choose_animation_scheme(scheme, overlay, "T_GOTHIT", &files.0);
    let path = get_animation_path(scheme, "T_GOTHIT");
    let resume = controller.current().clone();
    controller.queue_next(handles.get_animation_handle(asset_server, &path), true);
    // The npc can be despawned before this applies (killed by another hit the
    // same frame), a reaction on a corpse is pointless
    commands.entity(npc).try_insert(NpcHitReaction { resume });
}

/// Restores the interrupted animation when the `T_GOTHIT` hit reaction of an
/// npc ends (see [`play_hit_reaction`]). Without a matching end (the reaction
/// switch was dropped, e.g. the animation asset was still loading) the next
/// end of any animation clears the stale flag.
pub(crate) fn npc_hit_reaction_end(
    mut ends: MessageReader<AnimationEnds>,
    mut commands: Commands,
    mut npcs: Query<(Entity, &NpcHitReaction, &mut AnimationController)>,
) {
    for end in ends.read() {
        let Ok((entity, reaction, mut controller)) = npcs.get_mut(end.who) else {
            continue;
        };
        controller.queue_next(reaction.resume.clone(), true);
        // Silenced: the npc can be despawned before this applies (died the
        // same frame), removing the reaction from a corpse is pointless
        commands.queue_silenced(move |world: &mut World| {
            if let Ok(mut entity) = world.get_entity_mut(entity) {
                entity.remove::<NpcHitReaction>();
            }
        });
    }
}
