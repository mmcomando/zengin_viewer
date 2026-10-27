//! Semantic model of the G2 `C_Npc` members the host uses.

use super::access::ScriptVmAccess;
use super::ids::{FunctionId, StringId};
use super::registry::FieldSlot;

/// `C_Npc.aivar` size in G2.
pub const AIVAR_MAX: usize = 100;

/// `C_Npc` members modeled by the host.
#[derive(Debug, Clone)]
pub struct Npc {
    /// `C_Npc.name[0]`, `None` when unset.
    pub name: Option<StringId>,
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
    /// Experience points.
    pub exp: i32,
    /// Experience needed for the next level.
    pub exp_next: i32,
    /// Learning points.
    pub lp: i32,
    /// `C_Npc.wp`, current waypoint string id, `None` when unset.
    pub waypoint: Option<StringId>,
    /// `C_Npc.aivar`, AI variables.
    pub aivar: [i32; AIVAR_MAX],
    /// `C_Npc.daily_routine` function, `None` for the `0`/`1` sentinels.
    pub daily_routine: Option<FunctionId>,
}

impl Default for Npc {
    fn default() -> Self {
        Self {
            name: None,
            hitpoints: 0,
            hitpoints_max: 0,
            mana: 0,
            mana_max: 0,
            strength: 0,
            dexterity: 0,
            exp: 0,
            exp_next: 0,
            lp: 0,
            waypoint: None,
            aivar: [0; AIVAR_MAX],
            daily_routine: None,
        }
    }
}

mod offset {
    pub const NAME: u32 = 292;
    pub const ATTRIBUTE: u32 = 440;
    pub const DAILY_ROUTINE: u32 = 608;
    pub const AIVAR: u32 = 648;
    pub const WP: u32 = 1048;
    pub const EXP: u32 = 1068;
    pub const EXP_NEXT: u32 = 1072;
    pub const LP: u32 = 1076;
}

/// `(member name, element, byte offset, modeled elements)` for validation and
/// initial seeding.
pub(crate) const FIELD_SLOTS: &[FieldSlot] = &[
    FieldSlot {
        member: "name",
        element: 0,
        offset: offset::NAME,
        count: 1,
    },
    FieldSlot {
        member: "attribute",
        element: 0,
        offset: offset::ATTRIBUTE,
        count: 1,
    },
    FieldSlot {
        member: "attribute",
        element: 1,
        offset: offset::ATTRIBUTE,
        count: 1,
    },
    FieldSlot {
        member: "attribute",
        element: 2,
        offset: offset::ATTRIBUTE,
        count: 1,
    },
    FieldSlot {
        member: "attribute",
        element: 3,
        offset: offset::ATTRIBUTE,
        count: 1,
    },
    FieldSlot {
        member: "attribute",
        element: 4,
        offset: offset::ATTRIBUTE,
        count: 1,
    },
    FieldSlot {
        member: "attribute",
        element: 5,
        offset: offset::ATTRIBUTE,
        count: 1,
    },
    FieldSlot {
        member: "daily_routine",
        element: 0,
        offset: offset::DAILY_ROUTINE,
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
];

impl ScriptVmAccess for Npc {
    fn read(&self, offset: u32, element: u32) -> Option<i32> {
        let index = element as usize;
        match (offset, element) {
            (offset::NAME, 0) => Some(self.name.map_or(0, StringId::value)),
            (offset::ATTRIBUTE, 0) => Some(self.hitpoints),
            (offset::ATTRIBUTE, 1) => Some(self.hitpoints_max),
            (offset::ATTRIBUTE, 2) => Some(self.mana),
            (offset::ATTRIBUTE, 3) => Some(self.mana_max),
            (offset::ATTRIBUTE, 4) => Some(self.strength),
            (offset::ATTRIBUTE, 5) => Some(self.dexterity),
            (offset::DAILY_ROUTINE, 0) => Some(self.daily_routine.map_or(0, FunctionId::value)),
            (offset::AIVAR, _) => self.aivar.get(index).copied(),
            (offset::WP, 0) => Some(self.waypoint.map_or(0, StringId::value)),
            (offset::EXP, 0) => Some(self.exp),
            (offset::EXP_NEXT, 0) => Some(self.exp_next),
            (offset::LP, 0) => Some(self.lp),
            _ => None,
        }
    }

    fn write(&mut self, offset: u32, element: u32, value: i32) -> bool {
        let index = element as usize;
        match (offset, element) {
            (offset::NAME, 0) => {
                self.name = StringId::new(value);
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
            (offset::DAILY_ROUTINE, 0) => {
                self.daily_routine = if value > 1 {
                    FunctionId::new(value)
                } else {
                    None
                };
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
            _ => false,
        }
    }
}
