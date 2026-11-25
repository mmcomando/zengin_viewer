//! Overlay animation scheme state of npcs.

use std::collections::HashSet;
use std::sync::PoisonError;

use bevy::prelude::*;

use crate::zengin::{NpcHandle, WeaponClass};

use daedalus_typed::{TALENT_1H, TALENT_2H};

/// Overlay animation scheme currently applied to an npc (`Mdl_ApplyOverlayMds`).
#[derive(Component, Default)]
pub struct NpcOverlay(pub Option<String>);

/// Overlay animation scheme selected by the npc's melee talent skill.
#[derive(Component, Default, Debug, Clone, PartialEq, Eq)]
pub struct NpcTalentOverlay {
    /// Scheme of the 1H skill (`HUMANS_1HST1`/`HUMANS_1HST2`).
    pub one_hand: Option<String>,
    /// Scheme of the 2H skill (`HUMANS_2HST1`/`HUMANS_2HST2`).
    pub two_hand: Option<String>,
}

impl NpcTalentOverlay {
    /// Overlay scheme for a melee weapon class.
    pub(crate) fn for_class(&self, class: Option<WeaponClass>) -> Option<&str> {
        match class {
            Some(WeaponClass::TwoHanded) => self.two_hand.as_deref(),
            _ => self.one_hand.as_deref(),
        }
    }
}

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

/// Scheme of the 2H fighting style taught at `skill` level (`HUMANS_2HST1` for
/// the first level, `HUMANS_2HST2` for the master level).
pub(crate) fn two_hand_talent_overlay(skill: i32) -> Option<String> {
    match skill {
        0 => None,
        1 => Some("HUMANS_2HST1".to_string()),
        _ => Some("HUMANS_2HST2".to_string()),
    }
}

/// Talent overlays of the npc's current 1H and 2H skills.
pub(crate) fn talent_overlay_for(handle: &NpcHandle) -> NpcTalentOverlay {
    let (one_hand, two_hand) = {
        let runtime = handle
            .runtime()
            .read()
            .unwrap_or_else(PoisonError::into_inner);
        (
            runtime.talents.skill(TALENT_1H as i32),
            runtime.talents.skill(TALENT_2H as i32),
        )
    };
    NpcTalentOverlay {
        one_hand: one_hand_talent_overlay(one_hand),
        two_hand: two_hand_talent_overlay(two_hand),
    }
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

    #[test]
    fn two_hand_talent_selects_the_skill_overlay() {
        assert_eq!(two_hand_talent_overlay(0), None);
        assert_eq!(two_hand_talent_overlay(1), Some("HUMANS_2HST1".to_string()));
        assert_eq!(two_hand_talent_overlay(2), Some("HUMANS_2HST2".to_string()));
        assert_eq!(two_hand_talent_overlay(9), Some("HUMANS_2HST2".to_string()));
    }

    #[test]
    fn overlay_selection_follows_the_melee_class() {
        let overlay = NpcTalentOverlay {
            one_hand: Some("HUMANS_1HST1".to_string()),
            two_hand: Some("HUMANS_2HST1".to_string()),
        };
        assert_eq!(
            overlay.for_class(Some(WeaponClass::OneHanded)),
            Some("HUMANS_1HST1")
        );
        assert_eq!(
            overlay.for_class(Some(WeaponClass::TwoHanded)),
            Some("HUMANS_2HST1")
        );
        assert_eq!(overlay.for_class(None), Some("HUMANS_1HST1"));
    }
}
