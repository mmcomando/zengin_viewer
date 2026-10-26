/// Overlay change requested for an npc.
#[derive(Debug, Clone, Copy)]
pub enum OverlayRequest {
    /// Apply the overlay whose name is the given string symbol id.
    Apply(i32),
    /// Remove the current overlay.
    Remove,
}

/// Weapon mode of an npc, drives the idle animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponMode {
    /// No weapon drawn (fists or sheathed)
    Fists,
    /// Melee weapon drawn
    Melee,
    /// Ranged weapon (bow or crossbow) drawn
    Ranged,
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
