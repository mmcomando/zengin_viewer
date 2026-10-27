//! Semantic model of the G2 `C_Item` members the host uses.

use super::access::ScriptVmAccess;
use super::ids::StringId;
use super::registry::FieldSlot;

/// `C_Item` members modeled by the host.
#[derive(Debug, Default, Clone)]
pub struct Item {
    /// `C_Item.name`, `None` when unset.
    pub name: Option<StringId>,
    /// Base value.
    pub value: i32,
    /// Weight.
    pub weight: i32,
    /// Total damage.
    pub damage_total: i32,
    /// Equip requirement attributes.
    pub cond_atr: [i32; 3],
    /// Equip requirement values.
    pub cond_value: [i32; 3],
    /// Description string id, `None` when unset.
    pub description: Option<StringId>,
    /// Info box text string ids.
    pub text: [Option<StringId>; 6],
    /// Info box text counts.
    pub count: [i32; 6],
    /// Visual model string id.
    pub visual: Option<StringId>,
    /// Visual override string id.
    pub visual_change: Option<StringId>,
}

mod offset {
    pub const NAME: u32 = 292;
    pub const VALUE: u32 = 352;
    pub const WEIGHT: u32 = 348;
    pub const DAMAGE_TOTAL: u32 = 360;
    pub const COND_ATR: u32 = 436;
    pub const COND_VALUE: u32 = 448;
    pub const DESCRIPTION: u32 = 628;
    pub const TEXT: u32 = 648;
    pub const COUNT: u32 = 768;
    pub const VISUAL: u32 = 524;
    pub const VISUAL_CHANGE: u32 = 544;
}

pub(crate) const FIELD_SLOTS: &[FieldSlot] = &[
    FieldSlot {
        member: "name",
        element: 0,
        offset: offset::NAME,
        count: 1,
    },
    FieldSlot {
        member: "value",
        element: 0,
        offset: offset::VALUE,
        count: 1,
    },
    FieldSlot {
        member: "weight",
        element: 0,
        offset: offset::WEIGHT,
        count: 1,
    },
    FieldSlot {
        member: "damagetotal",
        element: 0,
        offset: offset::DAMAGE_TOTAL,
        count: 1,
    },
    FieldSlot {
        member: "cond_atr",
        element: 0,
        offset: offset::COND_ATR,
        count: 3,
    },
    FieldSlot {
        member: "cond_value",
        element: 0,
        offset: offset::COND_VALUE,
        count: 3,
    },
    FieldSlot {
        member: "description",
        element: 0,
        offset: offset::DESCRIPTION,
        count: 1,
    },
    FieldSlot {
        member: "text",
        element: 0,
        offset: offset::TEXT,
        count: 6,
    },
    FieldSlot {
        member: "count",
        element: 0,
        offset: offset::COUNT,
        count: 6,
    },
    FieldSlot {
        member: "visual",
        element: 0,
        offset: offset::VISUAL,
        count: 1,
    },
    FieldSlot {
        member: "visual_change",
        element: 0,
        offset: offset::VISUAL_CHANGE,
        count: 1,
    },
];

impl ScriptVmAccess for Item {
    fn read(&self, offset: u32, element: u32) -> Option<i32> {
        let index = element as usize;
        match (offset, element) {
            (offset::NAME, 0) => Some(self.name.map_or(0, StringId::value)),
            (offset::VALUE, 0) => Some(self.value),
            (offset::WEIGHT, 0) => Some(self.weight),
            (offset::DAMAGE_TOTAL, 0) => Some(self.damage_total),
            (offset::COND_ATR, _) => self.cond_atr.get(index).copied(),
            (offset::COND_VALUE, _) => self.cond_value.get(index).copied(),
            (offset::DESCRIPTION, 0) => Some(self.description.map_or(0, StringId::value)),
            (offset::TEXT, _) => self.text.get(index).map(|id| id.map_or(0, StringId::value)),
            (offset::COUNT, _) => self.count.get(index).copied(),
            (offset::VISUAL, 0) => Some(self.visual.map_or(0, StringId::value)),
            (offset::VISUAL_CHANGE, 0) => Some(self.visual_change.map_or(0, StringId::value)),
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
            (offset::VALUE, 0) => {
                self.value = value;
                true
            }
            (offset::WEIGHT, 0) => {
                self.weight = value;
                true
            }
            (offset::DAMAGE_TOTAL, 0) => {
                self.damage_total = value;
                true
            }
            (offset::COND_ATR, _) => write_slot(&mut self.cond_atr, index, value),
            (offset::COND_VALUE, _) => write_slot(&mut self.cond_value, index, value),
            (offset::DESCRIPTION, 0) => {
                self.description = StringId::new(value);
                true
            }
            (offset::TEXT, _) => write_string_slot(&mut self.text, index, value),
            (offset::COUNT, _) => write_slot(&mut self.count, index, value),
            (offset::VISUAL, 0) => {
                self.visual = StringId::new(value);
                true
            }
            (offset::VISUAL_CHANGE, 0) => {
                self.visual_change = StringId::new(value);
                true
            }
            _ => false,
        }
    }
}

fn write_slot<T: Copy>(slots: &mut [T], index: usize, value: T) -> bool {
    let Some(slot) = slots.get_mut(index) else {
        return false;
    };
    *slot = value;
    true
}

fn write_string_slot(slots: &mut [Option<StringId>], index: usize, value: i32) -> bool {
    let Some(slot) = slots.get_mut(index) else {
        return false;
    };
    *slot = StringId::new(value);
    true
}
