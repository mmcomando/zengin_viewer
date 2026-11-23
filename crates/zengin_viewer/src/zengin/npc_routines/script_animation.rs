//! Applies animation requests produced by the script vm to npc visuals.

use bevy::prelude::*;

use crate::gameplay::objects::NpcModel;
use crate::graphics::{AnimationController, AutoAnimSequencer};
use crate::zengin::common::{choose_animation_scheme, get_animation_path};
use crate::zengin::instance_index::InstanceIndex;
use crate::zengin::npc_routines::overlay::{AnimationFiles, NpcOverlay};
use crate::zengin::resources::CachedAssetHandles;
use crate::zengin::script::state::AnimationChange;
use crate::zengin::script::string_value::StringCanon;

use super::routines::{STANDING_ANIMATION, has_animation};
use crate::perf_debug;

/// Applies the `AI_PlayAni(BS)` requests drained by the script bridge. Requests
/// are applied in order, so a later one overrides an earlier one.
pub(crate) fn apply_scripted_animations(
    mut changes: MessageReader<AnimationChange>,
    index: Res<InstanceIndex>,
    files: Res<AnimationFiles>,
    mut handles: ResMut<CachedAssetHandles>,
    asset_server: Res<AssetServer>,
    mut npcs: Query<(
        &NpcModel,
        Option<&NpcOverlay>,
        &mut AnimationController,
        &mut AutoAnimSequencer,
    )>,
) {
    let _perf = perf_debug!();
    for change in changes.read() {
        for &entity in index.npcs(change.instance_id) {
            let Ok((model, overlay, mut controller, mut sequencer)) = npcs.get_mut(entity) else {
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
            let Some(animation) = resolve_script_animation(model, overlay, &files, &animation)
            else {
                // Scripts request animations for every scheme, not all of them
                // exist in the archive (e.g. `SHADOW-T_PERCEPTION`). The engine
                // silently ignores such a request.
                warn!(
                    "npc({}) scripted animation({animation}) is not in the anim archive, skipped, scheme({scheme}), model({})",
                    change.instance_id, model.body_model
                );
                continue;
            };
            let overlay = overlay.and_then(|overlay| overlay.0.as_deref());
            let scheme = choose_animation_scheme(scheme, overlay, &animation, &files.0);
            let path = get_animation_path(scheme, &animation);
            // A scripted pose loops on its own; the graph must not switch it
            // back to the standing idle when it ends.
            sequencer.suspend();
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

/// Roam poses the monster routines pick from at random (`Hlp_Random(3)`). A
/// scheme may ship only some of them (WOLF has 2 and 3, ORC 1 and 2), so a
/// missing variant falls back to any other variant of the same scheme.
const ROAM_ANIMATIONS: [&str; 3] = ["R_ROAM1", "R_ROAM2", "R_ROAM3"];

/// Resolves a requested animation against the archive, substituting an
/// available `R_ROAM` variant when the requested one is missing. `None` when
/// nothing usable exists for the scheme.
fn resolve_script_animation(
    model: &NpcModel,
    overlay: Option<&NpcOverlay>,
    files: &AnimationFiles,
    animation: &str,
) -> Option<String> {
    if has_animation(model, overlay, files, animation) {
        return Some(animation.to_string());
    }
    let is_roam = animation
        .get(..6)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("R_ROAM"));
    if !is_roam {
        return None;
    }
    ROAM_ANIMATIONS
        .iter()
        .find(|name| has_animation(model, overlay, files, name))
        .map(|name| (*name).to_string())
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    fn model() -> NpcModel {
        NpcModel {
            animation_scheme: Some("WOLF".to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn missing_roam_variant_falls_back_to_an_available_one() {
        let files = AnimationFiles(HashSet::from([
            "WOLF-R_ROAM2.MAN".to_string(),
            "WOLF-R_ROAM3.MAN".to_string(),
        ]));

        let resolved = resolve_script_animation(&model(), None, &files, "r_roam1");

        assert_eq!(resolved.as_deref(), Some("R_ROAM2"));
    }

    #[test]
    fn roam_without_any_variant_is_not_substituted() {
        let files = AnimationFiles(HashSet::from(["WOLF-S_FISTRUN.MAN".to_string()]));

        let resolved = resolve_script_animation(&model(), None, &files, "r_roam3");

        assert_eq!(resolved, None);
    }

    #[test]
    fn missing_non_roam_animation_is_not_substituted() {
        let files = AnimationFiles(HashSet::from(["WOLF-R_ROAM2.MAN".to_string()]));

        let resolved = resolve_script_animation(&model(), None, &files, "t_perception");

        assert_eq!(resolved, None);
    }
}
