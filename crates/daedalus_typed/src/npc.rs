//! Semantic model of the G2 `C_Npc` members the host uses.

use super::access::ScriptVmAccess;
use super::damage::DAMAGE_TYPES;
use super::ids::{FunctionId, StringId};
use super::registry::FieldSlot;

/// `C_Npc.aivar` size in G2.
pub const AIVAR_MAX: usize = 100;

/// `C_Npc.hitchance` size in G2.
pub const HITCHANCE_MAX: usize = 5;

/// `C_Npc.mission` size in G2.
pub const MISSION_MAX: usize = 5;

/// `ATR_HITPOINTS` attribute slot.
pub const ATR_HITPOINTS: i32 = 0;
/// `ATR_HITPOINTS_MAX` attribute slot.
pub const ATR_HITPOINTS_MAX: i32 = 1;
/// `ATR_MANA` attribute slot.
pub const ATR_MANA: i32 = 2;
/// `ATR_MANA_MAX` attribute slot.
pub const ATR_MANA_MAX: i32 = 3;
/// `ATR_STRENGTH` attribute slot.
pub const ATR_STRENGTH: i32 = 4;
/// `ATR_DEXTERITY` attribute slot.
pub const ATR_DEXTERITY: i32 = 5;
/// `ATR_REGENERATEHP` attribute slot.
pub const ATR_REGENERATE_HP: i32 = 6;
/// `ATR_REGENERATEMANA` attribute slot.
pub const ATR_REGENERATE_MANA: i32 = 7;
/// `ATR_INDEX_MAX`, one past the last attribute slot.
pub const ATR_INDEX_MAX: i32 = 8;

/// `TALENT_UNKNOWN`, the unarmed `C_Npc.hitchance` slot.
pub const TALENT_UNKNOWN: usize = 0;
/// `NPC_TALENT_1H`, the one-handed `C_Npc.hitchance` slot.
pub const TALENT_1H: usize = 1;
/// `NPC_TALENT_2H`, the two-handed `C_Npc.hitchance` slot.
pub const TALENT_2H: usize = 2;

/// `C_Npc` members modeled by the host.
#[derive(Debug, Clone)]
pub struct Npc {
    /// `C_Npc.id`.
    pub id: i32,
    /// `C_Npc.name[0]`, `None` when unset.
    pub name: Option<StringId>,
    /// Additional `C_Npc.name` entries, `None` when unset.
    pub name_extra: [Option<StringId>; 4],
    /// `C_Npc.slot`, `None` when unset.
    pub slot: Option<StringId>,
    /// `C_Npc.effect`, `None` when unset.
    pub effect: Option<StringId>,
    /// `C_Npc.npctype`.
    pub npctype: i32,
    /// `C_Npc.flags`.
    pub flags: i32,
    /// `C_Npc.attribute[ATR_HITPOINTS]`.
    pub hitpoints: i32,
    /// `C_Npc.attribute[ATR_HITPOINTS_MAX]`.
    pub hitpoints_max: i32,
    /// `C_Npc.attribute[ATR_MANA]`.
    pub mana: i32,
    /// `C_Npc.attribute[ATR_MANA_MAX]`.
    pub mana_max: i32,
    /// `C_Npc.attribute[ATR_STRENGTH]`.
    pub strength: i32,
    /// `C_Npc.attribute[ATR_DEXTERITY]`.
    pub dexterity: i32,
    /// `C_Npc.attribute[ATR_REGENERATEHP]`.
    pub regenerate_hp: i32,
    /// `C_Npc.attribute[ATR_REGENERATEMANA]`.
    pub regenerate_mana: i32,
    /// `C_Npc.hitchance`.
    pub hitchance: [i32; HITCHANCE_MAX],
    /// `C_Npc.protection`.
    pub protection: [i32; DAMAGE_TYPES],
    /// `C_Npc.damage`.
    pub damage: [i32; DAMAGE_TYPES],
    /// `C_Npc.damagetype`.
    pub damagetype: i32,
    /// `C_Npc.guild`.
    pub guild: i32,
    /// `C_Npc.level`.
    pub level: i32,
    /// `C_Npc.mission` function ids, `None` when unset.
    pub mission: [Option<FunctionId>; MISSION_MAX],
    /// `C_Npc.fight_tactic`.
    pub fight_tactic: i32,
    /// `C_Npc.weapon`.
    pub weapon: i32,
    /// `C_Npc.voice`.
    pub voice: i32,
    /// `C_Npc.voicepitch`.
    pub voicepitch: i32,
    /// `C_Npc.bodymass`.
    pub bodymass: i32,
    /// `C_Npc.daily_routine` function, `None` for the `0`/`1` sentinels.
    pub daily_routine: Option<FunctionId>,
    /// `C_Npc.start_aistate` function, `None` for the `0`/`1` sentinels.
    pub start_aistate: Option<FunctionId>,
    /// `C_Npc.spawnpoint`, `None` when unset.
    pub spawnpoint: Option<StringId>,
    /// `C_Npc.spawndelay`.
    pub spawndelay: i32,
    /// `C_Npc.senses`.
    pub senses: i32,
    /// `C_Npc.senses_range`.
    pub senses_range: i32,
    /// `C_Npc.aivar`, AI variables.
    pub aivar: [i32; AIVAR_MAX],
    /// `C_Npc.wp`, current waypoint string id, `None` when unset.
    pub waypoint: Option<StringId>,
    /// Experience points.
    pub exp: i32,
    /// Experience needed for the next level.
    pub exp_next: i32,
    /// Learning points.
    pub lp: i32,
    /// `C_Npc.bodystateinterruptableoverride`.
    pub bodystateinterruptableoverride: i32,
    /// `C_Npc.nofocus`.
    pub nofocus: i32,
}

impl Npc {
    /// Applies `delta` to one `ATR_*` attribute slot. Hitpoints and mana clamp
    /// to their maxima, every slot clamps at zero. Returns `false` for an
    /// unknown slot.
    pub fn change_attribute(&mut self, atr: i32, delta: i32) -> bool {
        match atr {
            ATR_HITPOINTS => {
                self.hitpoints = (self.hitpoints + delta).clamp(0, self.hitpoints_max);
            }
            ATR_HITPOINTS_MAX => self.hitpoints_max = (self.hitpoints_max + delta).max(0),
            ATR_MANA => self.mana = (self.mana + delta).clamp(0, self.mana_max),
            ATR_MANA_MAX => self.mana_max = (self.mana_max + delta).max(0),
            ATR_STRENGTH => self.strength = (self.strength + delta).max(0),
            ATR_DEXTERITY => self.dexterity = (self.dexterity + delta).max(0),
            ATR_REGENERATE_HP => self.regenerate_hp = (self.regenerate_hp + delta).max(0),
            ATR_REGENERATE_MANA => {
                self.regenerate_mana = (self.regenerate_mana + delta).max(0);
            }
            _ => return false,
        }
        true
    }
}

impl Default for Npc {
    fn default() -> Self {
        Self {
            id: 0,
            name: None,
            name_extra: [None; 4],
            slot: None,
            effect: None,
            npctype: 0,
            flags: 0,
            hitpoints: 0,
            hitpoints_max: 0,
            mana: 0,
            mana_max: 0,
            strength: 0,
            dexterity: 0,
            regenerate_hp: 0,
            regenerate_mana: 0,
            hitchance: [0; HITCHANCE_MAX],
            protection: [0; DAMAGE_TYPES],
            damage: [0; DAMAGE_TYPES],
            damagetype: 0,
            guild: 0,
            level: 0,
            mission: [None; MISSION_MAX],
            fight_tactic: 0,
            weapon: 0,
            voice: 0,
            voicepitch: 0,
            bodymass: 0,
            daily_routine: None,
            start_aistate: None,
            spawnpoint: None,
            spawndelay: 0,
            senses: 0,
            senses_range: 0,
            aivar: [0; AIVAR_MAX],
            waypoint: None,
            exp: 0,
            exp_next: 0,
            lp: 0,
            bodystateinterruptableoverride: 0,
            nofocus: 0,
        }
    }
}

mod offset {
    pub const ID: u32 = 288;
    pub const NAME: u32 = 292;
    pub const SLOT: u32 = 392;
    pub const EFFECT: u32 = 412;
    pub const NPCTYPE: u32 = 432;
    pub const FLAGS: u32 = 436;
    pub const ATTRIBUTE: u32 = 440;
    pub const HITCHANCE: u32 = 472;
    pub const PROTECTION: u32 = 492;
    pub const DAMAGE: u32 = 524;
    pub const DAMAGETYPE: u32 = 556;
    pub const GUILD: u32 = 560;
    pub const LEVEL: u32 = 564;
    pub const MISSION: u32 = 568;
    pub const FIGHT_TACTIC: u32 = 588;
    pub const WEAPON: u32 = 592;
    pub const VOICE: u32 = 596;
    pub const VOICEPITCH: u32 = 600;
    pub const BODYMASS: u32 = 604;
    pub const DAILY_ROUTINE: u32 = 608;
    pub const START_AISTATE: u32 = 612;
    pub const SPAWNPOINT: u32 = 616;
    pub const SPAWNDELAY: u32 = 636;
    pub const SENSES: u32 = 640;
    pub const SENSES_RANGE: u32 = 644;
    pub const AIVAR: u32 = 648;
    pub const WP: u32 = 1048;
    pub const EXP: u32 = 1068;
    pub const EXP_NEXT: u32 = 1072;
    pub const LP: u32 = 1076;
    pub const BODYSTATE_INTERRUPTABLE_OVERRIDE: u32 = 1080;
    pub const NOFOCUS: u32 = 1084;
}

/// `(member name, element, byte offset, modeled elements)` for validation and
/// value routing.
pub(crate) const FIELD_SLOTS: &[FieldSlot] = &[
    FieldSlot {
        member: "id",
        element: 0,
        offset: offset::ID,
        count: 1,
    },
    FieldSlot {
        member: "name",
        element: 0,
        offset: offset::NAME,
        count: 1,
    },
    FieldSlot {
        member: "name",
        element: 1,
        offset: offset::NAME,
        count: 4,
    },
    FieldSlot {
        member: "slot",
        element: 0,
        offset: offset::SLOT,
        count: 1,
    },
    FieldSlot {
        member: "effect",
        element: 0,
        offset: offset::EFFECT,
        count: 1,
    },
    FieldSlot {
        member: "npctype",
        element: 0,
        offset: offset::NPCTYPE,
        count: 1,
    },
    FieldSlot {
        member: "flags",
        element: 0,
        offset: offset::FLAGS,
        count: 1,
    },
    FieldSlot {
        member: "attribute",
        element: 0,
        offset: offset::ATTRIBUTE,
        count: 6,
    },
    FieldSlot {
        member: "attribute",
        element: 6,
        offset: offset::ATTRIBUTE,
        count: 2,
    },
    FieldSlot {
        member: "hitchance",
        element: 0,
        offset: offset::HITCHANCE,
        count: 5,
    },
    FieldSlot {
        member: "protection",
        element: 0,
        offset: offset::PROTECTION,
        count: 8,
    },
    FieldSlot {
        member: "damage",
        element: 0,
        offset: offset::DAMAGE,
        count: 8,
    },
    FieldSlot {
        member: "damagetype",
        element: 0,
        offset: offset::DAMAGETYPE,
        count: 1,
    },
    FieldSlot {
        member: "guild",
        element: 0,
        offset: offset::GUILD,
        count: 1,
    },
    FieldSlot {
        member: "level",
        element: 0,
        offset: offset::LEVEL,
        count: 1,
    },
    FieldSlot {
        member: "mission",
        element: 0,
        offset: offset::MISSION,
        count: 5,
    },
    FieldSlot {
        member: "fight_tactic",
        element: 0,
        offset: offset::FIGHT_TACTIC,
        count: 1,
    },
    FieldSlot {
        member: "weapon",
        element: 0,
        offset: offset::WEAPON,
        count: 1,
    },
    FieldSlot {
        member: "voice",
        element: 0,
        offset: offset::VOICE,
        count: 1,
    },
    FieldSlot {
        member: "voicepitch",
        element: 0,
        offset: offset::VOICEPITCH,
        count: 1,
    },
    FieldSlot {
        member: "bodymass",
        element: 0,
        offset: offset::BODYMASS,
        count: 1,
    },
    FieldSlot {
        member: "daily_routine",
        element: 0,
        offset: offset::DAILY_ROUTINE,
        count: 1,
    },
    FieldSlot {
        member: "start_aistate",
        element: 0,
        offset: offset::START_AISTATE,
        count: 1,
    },
    FieldSlot {
        member: "spawnpoint",
        element: 0,
        offset: offset::SPAWNPOINT,
        count: 1,
    },
    FieldSlot {
        member: "spawndelay",
        element: 0,
        offset: offset::SPAWNDELAY,
        count: 1,
    },
    FieldSlot {
        member: "senses",
        element: 0,
        offset: offset::SENSES,
        count: 1,
    },
    FieldSlot {
        member: "senses_range",
        element: 0,
        offset: offset::SENSES_RANGE,
        count: 1,
    },
    FieldSlot {
        member: "aivar",
        element: 0,
        offset: offset::AIVAR,
        count: AIVAR_MAX as u32,
    },
    FieldSlot {
        member: "wp",
        element: 0,
        offset: offset::WP,
        count: 1,
    },
    FieldSlot {
        member: "exp",
        element: 0,
        offset: offset::EXP,
        count: 1,
    },
    FieldSlot {
        member: "exp_next",
        element: 0,
        offset: offset::EXP_NEXT,
        count: 1,
    },
    FieldSlot {
        member: "lp",
        element: 0,
        offset: offset::LP,
        count: 1,
    },
    FieldSlot {
        member: "bodystateinterruptableoverride",
        element: 0,
        offset: offset::BODYSTATE_INTERRUPTABLE_OVERRIDE,
        count: 1,
    },
    FieldSlot {
        member: "nofocus",
        element: 0,
        offset: offset::NOFOCUS,
        count: 1,
    },
];

impl ScriptVmAccess for Npc {
    fn read(&self, offset: u32, element: u32) -> Option<i32> {
        let index = element as usize;
        match (offset, element) {
            (offset::ID, 0) => Some(self.id),
            (offset::NAME, 0) => Some(self.name.map_or(0, StringId::value)),
            (offset::NAME, _) => self
                .name_extra
                .get(index.saturating_sub(1))
                .map(|id| id.map_or(0, StringId::value)),
            (offset::SLOT, 0) => Some(self.slot.map_or(0, StringId::value)),
            (offset::EFFECT, 0) => Some(self.effect.map_or(0, StringId::value)),
            (offset::NPCTYPE, 0) => Some(self.npctype),
            (offset::FLAGS, 0) => Some(self.flags),
            (offset::ATTRIBUTE, 0) => Some(self.hitpoints),
            (offset::ATTRIBUTE, 1) => Some(self.hitpoints_max),
            (offset::ATTRIBUTE, 2) => Some(self.mana),
            (offset::ATTRIBUTE, 3) => Some(self.mana_max),
            (offset::ATTRIBUTE, 4) => Some(self.strength),
            (offset::ATTRIBUTE, 5) => Some(self.dexterity),
            (offset::ATTRIBUTE, 6) => Some(self.regenerate_hp),
            (offset::ATTRIBUTE, 7) => Some(self.regenerate_mana),
            (offset::HITCHANCE, _) => self.hitchance.get(index).copied(),
            (offset::PROTECTION, _) => self.protection.get(index).copied(),
            (offset::DAMAGE, _) => self.damage.get(index).copied(),
            (offset::DAMAGETYPE, 0) => Some(self.damagetype),
            (offset::GUILD, 0) => Some(self.guild),
            (offset::LEVEL, 0) => Some(self.level),
            (offset::MISSION, _) => self
                .mission
                .get(index)
                .map(|id| id.map_or(0, FunctionId::value)),
            (offset::FIGHT_TACTIC, 0) => Some(self.fight_tactic),
            (offset::WEAPON, 0) => Some(self.weapon),
            (offset::VOICE, 0) => Some(self.voice),
            (offset::VOICEPITCH, 0) => Some(self.voicepitch),
            (offset::BODYMASS, 0) => Some(self.bodymass),
            (offset::DAILY_ROUTINE, 0) => Some(self.daily_routine.map_or(0, FunctionId::value)),
            (offset::START_AISTATE, 0) => Some(self.start_aistate.map_or(0, FunctionId::value)),
            (offset::SPAWNPOINT, 0) => Some(self.spawnpoint.map_or(0, StringId::value)),
            (offset::SPAWNDELAY, 0) => Some(self.spawndelay),
            (offset::SENSES, 0) => Some(self.senses),
            (offset::SENSES_RANGE, 0) => Some(self.senses_range),
            (offset::AIVAR, _) => self.aivar.get(index).copied(),
            (offset::WP, 0) => Some(self.waypoint.map_or(0, StringId::value)),
            (offset::EXP, 0) => Some(self.exp),
            (offset::EXP_NEXT, 0) => Some(self.exp_next),
            (offset::LP, 0) => Some(self.lp),
            (offset::BODYSTATE_INTERRUPTABLE_OVERRIDE, 0) => {
                Some(self.bodystateinterruptableoverride)
            }
            (offset::NOFOCUS, 0) => Some(self.nofocus),
            _ => None,
        }
    }

    fn write(&mut self, offset: u32, element: u32, value: i32) -> bool {
        let index = element as usize;
        match (offset, element) {
            (offset::ID, 0) => {
                self.id = value;
                true
            }
            (offset::NAME, 0) => {
                self.name = StringId::new(value);
                true
            }
            (offset::NAME, _) => {
                let Some(slot) = self.name_extra.get_mut(index.saturating_sub(1)) else {
                    return false;
                };
                *slot = StringId::new(value);
                true
            }
            (offset::SLOT, 0) => {
                self.slot = StringId::new(value);
                true
            }
            (offset::EFFECT, 0) => {
                self.effect = StringId::new(value);
                true
            }
            (offset::NPCTYPE, 0) => {
                self.npctype = value;
                true
            }
            (offset::FLAGS, 0) => {
                self.flags = value;
                true
            }
            (offset::ATTRIBUTE, 0) => {
                self.hitpoints = value;
                true
            }
            (offset::ATTRIBUTE, 1) => {
                self.hitpoints_max = value;
                true
            }
            (offset::ATTRIBUTE, 2) => {
                self.mana = value;
                true
            }
            (offset::ATTRIBUTE, 3) => {
                self.mana_max = value;
                true
            }
            (offset::ATTRIBUTE, 4) => {
                self.strength = value;
                true
            }
            (offset::ATTRIBUTE, 5) => {
                self.dexterity = value;
                true
            }
            (offset::ATTRIBUTE, 6) => {
                self.regenerate_hp = value;
                true
            }
            (offset::ATTRIBUTE, 7) => {
                self.regenerate_mana = value;
                true
            }
            (offset::HITCHANCE, _) => write_slot(&mut self.hitchance, index, value),
            (offset::PROTECTION, _) => write_slot(&mut self.protection, index, value),
            (offset::DAMAGE, _) => write_slot(&mut self.damage, index, value),
            (offset::DAMAGETYPE, 0) => {
                self.damagetype = value;
                true
            }
            (offset::GUILD, 0) => {
                self.guild = value;
                true
            }
            (offset::LEVEL, 0) => {
                self.level = value;
                true
            }
            (offset::MISSION, _) => write_function_slot(&mut self.mission, index, value),
            (offset::FIGHT_TACTIC, 0) => {
                self.fight_tactic = value;
                true
            }
            (offset::WEAPON, 0) => {
                self.weapon = value;
                true
            }
            (offset::VOICE, 0) => {
                self.voice = value;
                true
            }
            (offset::VOICEPITCH, 0) => {
                self.voicepitch = value;
                true
            }
            (offset::BODYMASS, 0) => {
                self.bodymass = value;
                true
            }
            (offset::DAILY_ROUTINE, 0) => {
                self.daily_routine = function_id(value);
                true
            }
            (offset::START_AISTATE, 0) => {
                self.start_aistate = function_id(value);
                true
            }
            (offset::SPAWNPOINT, 0) => {
                self.spawnpoint = StringId::new(value);
                true
            }
            (offset::SPAWNDELAY, 0) => {
                self.spawndelay = value;
                true
            }
            (offset::SENSES, 0) => {
                self.senses = value;
                true
            }
            (offset::SENSES_RANGE, 0) => {
                self.senses_range = value;
                true
            }
            (offset::AIVAR, _) => {
                let Some(slot) = self.aivar.get_mut(index) else {
                    return false;
                };
                *slot = value;
                true
            }
            (offset::WP, 0) => {
                self.waypoint = StringId::new(value);
                true
            }
            (offset::EXP, 0) => {
                self.exp = value;
                true
            }
            (offset::EXP_NEXT, 0) => {
                self.exp_next = value;
                true
            }
            (offset::LP, 0) => {
                self.lp = value;
                true
            }
            (offset::BODYSTATE_INTERRUPTABLE_OVERRIDE, 0) => {
                self.bodystateinterruptableoverride = value;
                true
            }
            (offset::NOFOCUS, 0) => {
                self.nofocus = value;
                true
            }
            _ => false,
        }
    }
}

fn function_id(value: i32) -> Option<FunctionId> {
    (value > 1).then(|| FunctionId::new(value)).flatten()
}

fn write_slot<T: Copy>(slots: &mut [T], index: usize, value: T) -> bool {
    let Some(slot) = slots.get_mut(index) else {
        return false;
    };
    *slot = value;
    true
}

fn write_function_slot(slots: &mut [Option<FunctionId>], index: usize, value: i32) -> bool {
    let Some(slot) = slots.get_mut(index) else {
        return false;
    };
    *slot = function_id(value);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn npc() -> Npc {
        Npc {
            hitpoints: 50,
            hitpoints_max: 100,
            mana: 20,
            mana_max: 40,
            strength: 10,
            dexterity: 10,
            regenerate_hp: 1,
            regenerate_mana: 1,
            ..Default::default()
        }
    }

    #[test]
    fn hitpoints_change_by_delta_and_clamp() {
        let mut npc = npc();
        assert!(npc.change_attribute(ATR_HITPOINTS, -10));
        assert_eq!(npc.hitpoints, 40);
        npc.change_attribute(ATR_HITPOINTS, 500);
        assert_eq!(npc.hitpoints, 100);
        npc.change_attribute(ATR_HITPOINTS, -500);
        assert_eq!(npc.hitpoints, 0);
    }

    #[test]
    fn mana_and_max_attributes_clamp_at_zero() {
        let mut npc = npc();
        npc.change_attribute(ATR_MANA, -500);
        assert_eq!(npc.mana, 0);
        npc.change_attribute(ATR_MANA_MAX, -500);
        assert_eq!(npc.mana_max, 0);
        npc.change_attribute(ATR_HITPOINTS_MAX, -500);
        assert_eq!(npc.hitpoints_max, 0);
    }

    #[test]
    fn strength_and_dexterity_change_by_delta() {
        let mut npc = npc();
        npc.change_attribute(ATR_STRENGTH, 5);
        npc.change_attribute(ATR_DEXTERITY, -20);
        assert_eq!(npc.strength, 15);
        assert_eq!(npc.dexterity, 0);
    }

    #[test]
    fn unknown_attribute_reports_no_change() {
        let mut npc = npc();
        assert!(!npc.change_attribute(99, 5));
    }
}
