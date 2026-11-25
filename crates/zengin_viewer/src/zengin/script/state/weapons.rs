use daedalus_typed::{ITEM_2HD_AXE, ITEM_2HD_SWD, ITEM_AXE, ITEM_DAG, ITEM_SWD};
use serde::{Deserialize, Serialize};

use bevy::prelude::Reflect;

/// `C_Item.mainflag` category bit of armor items (`ITEM_KAT_ARMOR`).
pub const ITEM_MAINFLAG_ARMOR: i32 = 16;

/// Overlay change requested for an npc.
#[derive(Debug, Clone, Copy)]
pub enum OverlayRequest {
    /// Apply the overlay whose name is the given string symbol id.
    Apply(i32),
    /// Remove the current overlay.
    Remove,
}

/// Weapon mode of an npc, drives the idle animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Reflect)]
pub enum WeaponMode {
    /// No weapon drawn (fists or sheathed)
    Fists,
    /// Melee weapon drawn
    Melee,
    /// Ranged weapon (bow or crossbow) drawn
    Ranged,
}

/// Melee weapon class of an equipped item, resolved from `C_Item.flags`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponClass {
    /// Dagger, sword or axe without the two-handed flag.
    OneHanded,
    /// Two-handed sword or axe.
    TwoHanded,
}

impl WeaponClass {
    /// Melee weapon class of an item's flags, `None` for a non-melee item.
    pub fn from_item_flags(flags: i32) -> Option<Self> {
        if flags & (ITEM_2HD_SWD | ITEM_2HD_AXE) != 0 {
            return Some(Self::TwoHanded);
        }
        (flags & (ITEM_DAG | ITEM_SWD | ITEM_AXE) != 0).then_some(Self::OneHanded)
    }
}

/// Kind of a weapon item, resolved from its instance symbol name prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponKind {
    /// `itmw_*` melee weapon
    Melee,
    /// `itrw_*` bow (not a crossbow)
    Bow,
    /// `itrw_*` crossbow
    Crossbow,
}

impl WeaponKind {
    /// Resolves the weapon kind of an item symbol name, `None` when the item
    /// is not a weapon.
    pub fn from_symbol_name(name: &str) -> Option<Self> {
        let name = name.to_lowercase();
        if name.starts_with("itmw_") {
            return Some(Self::Melee);
        }
        if name.starts_with("itrw_") {
            return Some(if name.contains("crossbow") {
                Self::Crossbow
            } else {
                Self::Bow
            });
        }
        None
    }

    /// Whether drawing this weapon kind switches the mode to [`WeaponMode::Ranged`].
    pub fn is_ranged(self) -> bool {
        matches!(self, Self::Bow | Self::Crossbow)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_flags_resolve_the_melee_weapon_class() {
        assert_eq!(
            WeaponClass::from_item_flags(ITEM_SWD),
            Some(WeaponClass::OneHanded)
        );
        assert_eq!(
            WeaponClass::from_item_flags(ITEM_AXE),
            Some(WeaponClass::OneHanded)
        );
        assert_eq!(
            WeaponClass::from_item_flags(ITEM_DAG),
            Some(WeaponClass::OneHanded)
        );
        assert_eq!(
            WeaponClass::from_item_flags(ITEM_2HD_SWD),
            Some(WeaponClass::TwoHanded)
        );
        assert_eq!(
            WeaponClass::from_item_flags(ITEM_2HD_AXE),
            Some(WeaponClass::TwoHanded)
        );
        assert_eq!(WeaponClass::from_item_flags(0), None);
    }
}
