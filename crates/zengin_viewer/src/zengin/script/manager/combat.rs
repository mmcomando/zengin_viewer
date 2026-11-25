//! Gothic II damage formula, detached from the script VM.
#![allow(dead_code)]

use crate::zengin::script::state::WeaponMode;
use crate::zengin::script::symbol_id::InstanceId;

use super::ScriptManager;

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

/// `GIL_SEPERATOR_HUM`, guilds below it are humans.
const GIL_SEPARATOR_HUMAN: i32 = 16;
/// `GIL_SEPERATOR_ORC`, guilds up to it are monsters.
const GIL_SEPARATOR_ORC: i32 = 58;

/// `ITEM_2HD_SWD` item flag.
const ITEM_2HD_SWD: i32 = 1 << 16;
/// `ITEM_2HD_AXE` item flag.
const ITEM_2HD_AXE: i32 = 1 << 17;

/// Upper bound of the melee hit-chance roll.
const HIT_CHANCE_BOUND: i32 = 100;

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

/// Kind of attack whose damage is being calculated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AttackKind {
    /// Melee swing, uses the attacker's drawn melee weapon when there is one.
    Melee,
    /// Ranged hit with the given weapon item.
    Ranged(InstanceId),
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

impl ScriptManager {
    /// Final damage of one hit, `None` when the attacker or victim has no typed
    /// `C_Npc` record.
    pub(crate) fn combat_damage(
        &mut self,
        attacker: InstanceId,
        victim: InstanceId,
        kind: AttackKind,
    ) -> Option<i32> {
        let stats = self.damage_stats(attacker)?;
        let protection = self.npc_data(victim.value() as u32)?.protection;
        match kind {
            AttackKind::Melee => {
                let weapon = self
                    .drawn_melee_weapon(attacker)
                    .and_then(|weapon| self.weapon_stats(weapon));
                let crit_roll = self.store.data_mut().combat_rng.roll(HIT_CHANCE_BOUND);
                Some(melee_damage(
                    &stats,
                    weapon.as_ref(),
                    &protection,
                    crit_roll,
                ))
            }
            AttackKind::Ranged(weapon) => {
                let weapon = self.weapon_stats(weapon)?;
                Some(ranged_damage(&stats, &weapon, &protection))
            }
        }
    }

    fn damage_stats(&self, npc: InstanceId) -> Option<DamageStats> {
        let data = self.npc_data(npc.value() as u32)?;
        Some(DamageStats {
            strength: data.strength,
            dexterity: data.dexterity,
            hitchance: data.hitchance,
            damage: data.damage,
            damagetype: data.damagetype,
            is_monster: is_monster_guild(data.guild),
        })
    }

    fn weapon_stats(&self, item: InstanceId) -> Option<WeaponStats> {
        let data = self.item_data(item.value() as u32)?;
        Some(WeaponStats {
            damage_total: data.damage_total,
            damage: data.damage,
            damagetype: data.damagetype,
            two_handed: data.flags & (ITEM_2HD_SWD | ITEM_2HD_AXE) != 0,
        })
    }

    fn drawn_melee_weapon(&self, npc: InstanceId) -> Option<InstanceId> {
        let state = self.npc_state(npc)?;
        (state.equipment.weapon_mode == Some(WeaponMode::Melee))
            .then_some(state.equipment.melee)
            .flatten()
    }
}

fn is_monster_guild(guild: i32) -> bool {
    GIL_SEPARATOR_HUMAN < guild && guild < GIL_SEPARATOR_ORC
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

    #[test]
    fn monster_guild_range_matches_the_engine() {
        assert!(!is_monster_guild(16));
        assert!(is_monster_guild(17));
        assert!(is_monster_guild(57));
        assert!(!is_monster_guild(58));
    }
}
