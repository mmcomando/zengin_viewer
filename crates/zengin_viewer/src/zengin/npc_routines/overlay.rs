//! Overlay animation scheme state of npcs.

use std::collections::HashSet;
use std::sync::PoisonError;

use bevy::prelude::*;

use crate::zengin::NpcHandle;

use daedalus_typed::TALENT_1H;

/// Overlay animation scheme currently applied to an npc (`Mdl_ApplyOverlayMds`).
#[derive(Component, Default)]
pub struct NpcOverlay(pub Option<String>);

/// Overlay animation scheme selected by the npc's weapon talent skill.
#[derive(Component, Default, Debug, Clone, PartialEq, Eq)]
pub struct NpcTalentOverlay(pub Option<String>);

/// File names in the compiled anim archive, used to resolve overlay animations.
#[derive(Resource, Default)]
pub struct AnimationFiles(pub HashSet<String>);

/// Strips a model file extension and uppercases: `Humans_Relaxed.mds` ->
/// `HUMANS_RELAXED`.
pub(crate) fn overlay_scheme(name: &str) -> String {
    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    stem.to_uppercase()
}

/// Scheme of the 1H fighting style taught at `skill` level (`HUMANS_1HST1` for
/// the first level, `HUMANS_1HST2` for the master level).
pub(crate) fn one_hand_talent_overlay(skill: i32) -> Option<String> {
    match skill {
        0 => None,
        1 => Some("HUMANS_1HST1".to_string()),
        _ => Some("HUMANS_1HST2".to_string()),
    }
}

/// Talent overlay of the npc's current 1H skill.
pub(crate) fn talent_overlay_for(handle: &NpcHandle) -> NpcTalentOverlay {
    let skill = handle
        .runtime()
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .talents
        .skill(TALENT_1H as i32);
    NpcTalentOverlay(one_hand_talent_overlay(skill))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_hand_talent_selects_the_skill_overlay() {
        assert_eq!(one_hand_talent_overlay(0), None);
        assert_eq!(one_hand_talent_overlay(1), Some("HUMANS_1HST1".to_string()));
        assert_eq!(one_hand_talent_overlay(2), Some("HUMANS_1HST2".to_string()));
        assert_eq!(one_hand_talent_overlay(9), Some("HUMANS_1HST2".to_string()));
    }
}
