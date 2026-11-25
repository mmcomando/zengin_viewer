//! Gothic II damage formula, detached from the script VM.
#![allow(dead_code)]

/// Number of `C_Npc.protection`, `C_Npc.damage` and `C_Item.damage` slots.
const DAMAGE_TYPES: usize = 8;

/// `DAM_INVALID`, the "no damage type" bitmask.
const DAM_INVALID: i32 = 0;
/// `DAM_BLUNT`, the melee default when an npc has no damage type.
const DAM_BLUNT: i32 = 1 << 1;

/// `TALENT_UNKNOWN`, the talent slot used by unarmed attacks.
const TALENT_UNKNOWN: usize = 0;
/// `NPC_TALENT_1H`.
const TALENT_1H: usize = 1;
/// `NPC_TALENT_2H`.
const TALENT_2H: usize = 2;

/// Gothic II floors every hit at this damage after protection.
const MIN_DAMAGE: i32 = 5;

/// Attacker stats of one damage calculation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct DamageStats {
    /// `C_Npc.attribute[ATR_STRENGTH]`.
    pub strength: i32,
    /// `C_Npc.attribute[ATR_DEXTERITY]`.
    pub dexterity: i32,
    /// `C_Npc.hitchance` indexed by talent.
    pub hitchance: [i32; 5],
    /// `C_Npc.damage` by damage type.
    pub damage: [i32; DAMAGE_TYPES],
    /// `C_Npc.damagetype` bitmask.
    pub damagetype: i32,
    /// Monster guild range (`Npc::isMonster`).
    pub is_monster: bool,
}

/// Drawn weapon stats of one damage calculation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct WeaponStats {
    /// `C_Item.damagetotal`.
    pub damage_total: i32,
    /// `C_Item.damage` by damage type.
    pub damage: [i32; DAMAGE_TYPES],
    /// `C_Item.damagetype` bitmask.
    pub damagetype: i32,
    /// `C_Item.flags` carries `ITEM_2HD_SWD` or `ITEM_2HD_AXE`.
    pub two_handed: bool,
}

/// Damage of one melee hit, `crit_roll` is the attacker's 0..100 roll before
/// the hit-chance check.
pub(crate) fn melee_damage(
    attacker: &DamageStats,
    weapon: Option<&WeaponStats>,
    protection: &[i32; DAMAGE_TYPES],
    crit_roll: i32,
) -> i32 {
    let dtype = damage_mask(weapon.map_or(attacker.damagetype, |weapon| weapon.damagetype));
    let talent = match weapon {
        Some(weapon) if weapon.two_handed => TALENT_2H,
        Some(_) => TALENT_1H,
        None => TALENT_UNKNOWN,
    };
    // Regular monsters without a weapon always land critical hits.
    let crit_roll = if attacker.is_monster && weapon.is_none() {
        -1
    } else {
        crit_roll
    };
    let crit = attacker.hitchance.get(talent).copied().unwrap_or(0) > crit_roll;
    let damage = combined_damage(attacker, weapon);

    let mut total = 0;
    for index in 0..DAMAGE_TYPES {
        if dtype & (1 << index) == 0 {
            continue;
        }
        let mut value = (attacker.strength + damage[index] - protection[index]).max(0);
        if !crit {
            value = (value - 1) / 10;
        }
        if protection[index] >= 0 {
            total += value;
        }
    }
    total.max(MIN_DAMAGE)
}

/// Damage of one ranged hit: dexterity plus weapon damage, no hit-chance roll.
pub(crate) fn ranged_damage(
    attacker: &DamageStats,
    weapon: &WeaponStats,
    protection: &[i32; DAMAGE_TYPES],
) -> i32 {
    let dtype = damage_mask(weapon.damagetype);
    let damage = combined_damage(attacker, Some(weapon));

    let mut total = 0;
    for index in 0..DAMAGE_TYPES {
        if dtype & (1 << index) == 0 {
            continue;
        }
        let value = (attacker.dexterity + damage[index] - protection[index]).max(0);
        if protection[index] >= 0 {
            total += value;
        }
    }
    total.max(MIN_DAMAGE)
}

/// An npc without a damage type fights with blunt damage.
fn damage_mask(damagetype: i32) -> i32 {
    if damagetype == DAM_INVALID {
        DAM_BLUNT
    } else {
        damagetype
    }
}

/// `C_Npc.damage` plus the drawn weapon.
fn combined_damage(attacker: &DamageStats, weapon: Option<&WeaponStats>) -> [i32; DAMAGE_TYPES] {
    let mut damage = attacker.damage;
    let Some(weapon) = weapon else {
        return damage;
    };
    let mask = damage_mask(weapon.damagetype);
    for index in 0..DAMAGE_TYPES {
        damage[index] += weapon.damage[index];
        if mask & (1 << index) != 0 {
            damage[index] += weapon.damage_total;
        }
    }
    damage
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAM_EDGE: i32 = 1 << 2;

    fn attacker() -> DamageStats {
        DamageStats {
            strength: 10,
            dexterity: 10,
            damagetype: DAM_EDGE,
            ..Default::default()
        }
    }

    fn weapon() -> WeaponStats {
        WeaponStats {
            damage_total: 20,
            damagetype: DAM_EDGE,
            ..Default::default()
        }
    }

    fn protection() -> [i32; DAMAGE_TYPES] {
        [0; DAMAGE_TYPES]
    }

    #[test]
    fn melee_adds_strength_and_weapon_damage() {
        let attacker = DamageStats {
            hitchance: [0, 100, 0, 0, 0],
            ..attacker()
        };
        assert_eq!(
            melee_damage(&attacker, Some(&weapon()), &protection(), 50),
            30
        );
    }

    #[test]
    fn melee_subtracts_protection() {
        let attacker = DamageStats {
            hitchance: [0, 100, 0, 0, 0],
            ..attacker()
        };
        let mut protection = protection();
        protection[2] = 15;
        assert_eq!(
            melee_damage(&attacker, Some(&weapon()), &protection, 50),
            15
        );
    }

    #[test]
    fn melee_non_crit_reduces_to_a_tenth() {
        let attacker = DamageStats {
            strength: 100,
            hitchance: [0, 0, 0, 0, 0],
            ..attacker()
        };
        assert_eq!(
            melee_damage(&attacker, Some(&weapon()), &protection(), 0),
            11
        );
    }

    #[test]
    fn melee_floors_at_minimum_damage() {
        let attacker = DamageStats {
            hitchance: [0, 100, 0, 0, 0],
            ..attacker()
        };
        let mut protection = protection();
        protection[2] = 100;
        assert_eq!(melee_damage(&attacker, Some(&weapon()), &protection, 50), 5);
    }

    #[test]
    fn unarmed_monster_always_crits_with_natural_damage() {
        let attacker = DamageStats {
            strength: 30,
            damagetype: DAM_EDGE,
            is_monster: true,
            ..Default::default()
        };
        assert_eq!(melee_damage(&attacker, None, &protection(), 99), 30);
    }

    #[test]
    fn unarmed_human_falls_back_to_blunt_and_uses_talent_zero() {
        let attacker = DamageStats {
            strength: 10,
            hitchance: [100, 0, 0, 0, 0],
            damagetype: DAM_INVALID,
            damage: [0, 2, 0, 0, 0, 0, 0, 0],
            ..Default::default()
        };
        let mut protection = protection();
        protection[2] = 100;
        assert_eq!(melee_damage(&attacker, None, &protection, 50), 12);
    }

    #[test]
    fn two_handed_weapon_uses_the_two_handed_talent() {
        let attacker = DamageStats {
            hitchance: [0, 0, 100, 0, 0],
            ..attacker()
        };
        let weapon = WeaponStats {
            two_handed: true,
            ..weapon()
        };
        assert_eq!(
            melee_damage(&attacker, Some(&weapon), &protection(), 50),
            30
        );
    }

    #[test]
    fn weapon_damage_in_other_damage_types_is_ignored() {
        let attacker = DamageStats {
            hitchance: [0, 100, 0, 0, 0],
            ..attacker()
        };
        let weapon = WeaponStats {
            damage: [100, 0, 0, 0, 0, 0, 0, 0],
            ..weapon()
        };
        assert_eq!(
            melee_damage(&attacker, Some(&weapon), &protection(), 50),
            30
        );
    }

    #[test]
    fn ranged_uses_dexterity_instead_of_strength() {
        let attacker = DamageStats {
            strength: 100,
            dexterity: 10,
            ..attacker()
        };
        assert_eq!(ranged_damage(&attacker, &weapon(), &protection()), 30);
    }

    #[test]
    fn ranged_subtracts_protection_and_floors_at_minimum_damage() {
        let mut protection = protection();
        protection[2] = 100;
        assert_eq!(ranged_damage(&attacker(), &weapon(), &protection), 5);
    }
}
