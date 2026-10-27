//! Semantic model of the G2 `C_Info` dialog members.

use super::access::ScriptVmAccess;
use super::ids::{FunctionId, InstanceId, StringId};
use super::registry::FieldSlot;

/// `C_Info` members modeled by the host.
#[derive(Debug, Default, Clone)]
pub struct Info {
    /// Npc the info belongs to.
    pub npc: Option<InstanceId>,
    /// Order priority (`C_Info.nr`).
    pub order: i32,
    /// Whether the info is important.
    pub important: bool,
    /// Condition function, `None` when unset.
    pub condition: Option<FunctionId>,
    /// Information function, `None` when unset.
    pub information: Option<FunctionId>,
    /// Description string id, `None` when unset.
    pub description: Option<StringId>,
    /// Whether the info is a trade dialog.
    pub trade: bool,
    /// Whether the info is permanent.
    pub permanent: bool,
}

mod offset {
    pub const NPC: u32 = 0;
    pub const NR: u32 = 4;
    pub const IMPORTANT: u32 = 8;
    pub const CONDITION: u32 = 12;
    pub const INFORMATION: u32 = 16;
    pub const DESCRIPTION: u32 = 20;
    pub const TRADE: u32 = 40;
    pub const PERMANENT: u32 = 44;
}

pub(crate) const FIELD_SLOTS: &[FieldSlot] = &[
    FieldSlot {
        member: "npc",
        element: 0,
        offset: offset::NPC,
        count: 1,
    },
    FieldSlot {
        member: "nr",
        element: 0,
        offset: offset::NR,
        count: 1,
    },
    FieldSlot {
        member: "important",
        element: 0,
        offset: offset::IMPORTANT,
        count: 1,
    },
    FieldSlot {
        member: "condition",
        element: 0,
        offset: offset::CONDITION,
        count: 1,
    },
    FieldSlot {
        member: "information",
        element: 0,
        offset: offset::INFORMATION,
        count: 1,
    },
    FieldSlot {
        member: "description",
        element: 0,
        offset: offset::DESCRIPTION,
        count: 1,
    },
    FieldSlot {
        member: "trade",
        element: 0,
        offset: offset::TRADE,
        count: 1,
    },
    FieldSlot {
        member: "permanent",
        element: 0,
        offset: offset::PERMANENT,
        count: 1,
    },
];

impl ScriptVmAccess for Info {
    fn read(&self, offset: u32, element: u32) -> Option<i32> {
        match (offset, element) {
            (offset::NPC, 0) => Some(self.npc.map_or(0, InstanceId::value)),
            (offset::NR, 0) => Some(self.order),
            (offset::IMPORTANT, 0) => Some(i32::from(self.important)),
            (offset::CONDITION, 0) => Some(self.condition.map_or(0, FunctionId::value)),
            (offset::INFORMATION, 0) => Some(self.information.map_or(0, FunctionId::value)),
            (offset::DESCRIPTION, 0) => Some(self.description.map_or(0, StringId::value)),
            (offset::TRADE, 0) => Some(i32::from(self.trade)),
            (offset::PERMANENT, 0) => Some(i32::from(self.permanent)),
            _ => None,
        }
    }

    fn write(&mut self, offset: u32, element: u32, value: i32) -> bool {
        match (offset, element) {
            (offset::NPC, 0) => {
                self.npc = InstanceId::new(value);
                true
            }
            (offset::NR, 0) => {
                self.order = value;
                true
            }
            (offset::IMPORTANT, 0) => {
                self.important = value != 0;
                true
            }
            (offset::CONDITION, 0) => {
                self.condition = FunctionId::new(value);
                true
            }
            (offset::INFORMATION, 0) => {
                self.information = FunctionId::new(value);
                true
            }
            (offset::DESCRIPTION, 0) => {
                self.description = StringId::new(value);
                true
            }
            (offset::TRADE, 0) => {
                self.trade = value != 0;
                true
            }
            (offset::PERMANENT, 0) => {
                self.permanent = value != 0;
                true
            }
            _ => false,
        }
    }
}
