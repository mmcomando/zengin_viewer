//! Applies animation requests produced by the script vm to npc visuals.

use bevy::prelude::*;

use crate::gameplay::objects::NpcModel;
use crate::graphics::AnimationController;
use crate::zengin::common::{choose_animation_scheme, get_animation_path};
use crate::zengin::instance_index::InstanceIndex;
use crate::zengin::npc_routines::overlay::{AnimationFiles, NpcOverlay};
use crate::zengin::resources::CachedAssetHandles;
use crate::zengin::script::state::AnimationChange;
use crate::zengin::script::string_value::StringCanon;

use super::routines::STANDING_ANIMATION;

/// Applies the `AI_PlayAni(BS)` requests drained by the script bridge. Requests
/// are applied in order, so a later one overrides an earlier one.
pub(crate) fn apply_scripted_animations(
    mut changes: MessageReader<AnimationChange>,
    index: Res<InstanceIndex>,
    files: Res<AnimationFiles>,
    mut handles: ResMut<CachedAssetHandles>,
    asset_server: Res<AssetServer>,
    mut npcs: Query<(&NpcModel, Option<&NpcOverlay>, &mut AnimationController)>,
) {
    for change in changes.read() {
        for &entity in index.npcs(change.instance_id) {
            let Ok((model, overlay, mut controller)) = npcs.get_mut(entity) else {
                continue;
            };
            let Some(scheme) = model.animation_scheme.as_deref() else {
                continue;
            };
            let stand_animation = model
                .stand_animation
                .as_deref()
                .unwrap_or(STANDING_ANIMATION);
            let animation = choose_script_animation(&change.animation, stand_animation);
            let overlay = overlay.and_then(|overlay| overlay.0.as_deref());
            let scheme = choose_animation_scheme(scheme, overlay, &animation, &files.0);
            let path = get_animation_path(scheme, &animation);
            controller.queue_next(handles.get_animation_handle(&asset_server, &path), true);
        }
    }
}

/// Maps the requested animation to the looping pose the engine settles in. All
/// viewer animations loop, so a transition into a pose (`T_X_2_Y`,
/// `..._2_stand`) is replaced by the pose idle it leads to.
fn choose_script_animation(animation: &StringCanon, stand_animation: &str) -> String {
    // Requested ani names are canonically lowercased, poses compare lowercase.
    match animation.as_str().rsplit_once("_2_") {
        Some((_, "stand")) => stand_animation.to_string(),
        Some((_, pose)) => format!("S_{}", pose.to_uppercase()),
        None => animation.as_str().to_string(),
    }
}
