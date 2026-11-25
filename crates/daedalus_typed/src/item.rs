//! Semantic model of the G2 `C_Item` members the host uses.

use super::access::ScriptVmAccess;
use super::damage::DAMAGE_TYPES;
use super::ids::{FunctionId, StringId};
use super::registry::FieldSlot;

/// `C_Item.on_state` size in G2.
pub const ON_STATE_MAX: usize = 4;

/// `ITEM_DAG` item flag.
pub const ITEM_DAG: i32 = 1 << 13;
/// `ITEM_SWD` item flag.
pub const ITEM_SWD: i32 = 1 << 14;
/// `ITEM_AXE` item flag.
pub const ITEM_AXE: i32 = 1 << 15;
/// `ITEM_2HD_SWD` item flag.
pub const ITEM_2HD_SWD: i32 = 1 << 16;
/// `ITEM_2HD_AXE` item flag.
pub const ITEM_2HD_AXE: i32 = 1 << 17;
/// `ITEM_SHIELD` item flag.
pub const ITEM_SHIELD: i32 = 1 << 18;
/// `ITEM_BOW` item flag.
pub const ITEM_BOW: i32 = 1 << 19;
/// `ITEM_CROSSBOW` item flag.
pub const ITEM_CROSSBOW: i32 = 1 << 20;

/// `C_Item` members modeled by the host.
#[derive(Debug, Default, Clone)]
pub struct Item {
    /// `C_Item.id`.
    pub id: i32,
    /// `C_Item.name`, `None` when unset.
    pub name: Option<StringId>,
    /// `C_Item.nameid`, `None` when unset.
    pub nameid: Option<StringId>,
    /// Hitpoints of a usable item.
    pub hp: i32,
    /// Maximum hitpoints of a usable item.
    pub hp_max: i32,
    /// Item category flag.
    pub mainflag: i32,
    /// Item flags.
    pub flags: i32,
    /// Base value.
    pub value: i32,
    /// Weight.
    pub weight: i32,
    /// Damage type.
    pub damagetype: i32,
    /// Total damage.
    pub damage_total: i32,
    /// Damage by type.
    pub damage: [i32; DAMAGE_TYPES],
    /// Wear value.
    pub wear: i32,
    /// Protection by type.
    pub protection: [i32; DAMAGE_TYPES],
    /// Nutrition value.
    pub nutrition: i32,
    /// Equip requirement attributes.
    pub cond_atr: [i32; 3],
    /// Equip requirement values.
    pub cond_value: [i32; 3],
    /// Attribute changes when equipped.
    pub change_atr: [i32; 3],
    /// Attribute change values when equipped.
    pub change_value: [i32; 3],
    /// Magic function, `None` when unset.
    pub magic: Option<FunctionId>,
    /// Equip function, `None` when unset.
    pub on_equip: Option<FunctionId>,
    /// Unequip function, `None` when unset.
    pub on_unequip: Option<FunctionId>,
    /// State functions, `None` when unset.
    pub on_state: [Option<FunctionId>; ON_STATE_MAX],
    /// Owner function, `None` when unset.
    pub owner: Option<FunctionId>,
    /// Owner guild.
    pub ownerguild: i32,
    /// Disguise guild.
    pub disguiseguild: i32,
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
    /// Effect string id.
    pub effect: Option<StringId>,
    /// Visual skin index.
    pub visual_skin: i32,
    /// Scemename string id.
    pub scemename: Option<StringId>,
    /// Material index.
    pub material: i32,
    /// Munition item string id.
    pub munition: i32,
    /// Spell id.
    pub spell: i32,
    /// Range.
    pub range: i32,
    /// Magic circle requirement.
    pub mag_circle: i32,
    /// Inventory z bias.
    pub inv_zbias: i32,
    /// Inventory x rotation.
    pub inv_rotx: i32,
    /// Inventory y rotation.
    pub inv_roty: i32,
    /// Inventory z rotation.
    pub inv_rotz: i32,
    /// Inventory animation.
    pub inv_animate: i32,
}

mod offset {
    pub const ID: u32 = 288;
    pub const NAME: u32 = 292;
    pub const NAMEID: u32 = 312;
    pub const HP: u32 = 332;
    pub const HP_MAX: u32 = 336;
    pub const MAINFLAG: u32 = 340;
    pub const FLAGS: u32 = 344;
    pub const WEIGHT: u32 = 348;
    pub const VALUE: u32 = 352;
    pub const DAMAGETYPE: u32 = 356;
    pub const DAMAGE_TOTAL: u32 = 360;
    pub const DAMAGE: u32 = 364;
    pub const WEAR: u32 = 396;
    pub const PROTECTION: u32 = 400;
    pub const NUTRITION: u32 = 432;
    pub const COND_ATR: u32 = 436;
    pub const COND_VALUE: u32 = 448;
    pub const CHANGE_ATR: u32 = 460;
    pub const CHANGE_VALUE: u32 = 472;
    pub const MAGIC: u32 = 484;
    pub const ON_EQUIP: u32 = 488;
    pub const ON_UNEQUIP: u32 = 492;
    pub const ON_STATE: u32 = 496;
    pub const OWNER: u32 = 512;
    pub const OWNERGUILD: u32 = 516;
    pub const DISGUISEGUILD: u32 = 520;
    pub const VISUAL: u32 = 524;
    pub const VISUAL_CHANGE: u32 = 544;
    pub const EFFECT: u32 = 564;
    pub const VISUAL_SKIN: u32 = 584;
    pub const SCEMENAME: u32 = 588;
    pub const MATERIAL: u32 = 608;
    pub const MUNITION: u32 = 612;
    pub const SPELL: u32 = 616;
    pub const RANGE: u32 = 620;
    pub const MAG_CIRCLE: u32 = 624;
    pub const DESCRIPTION: u32 = 628;
    pub const TEXT: u32 = 648;
    pub const COUNT: u32 = 768;
    pub const INV_ZBIAS: u32 = 792;
    pub const INV_ROTX: u32 = 796;
    pub const INV_ROTY: u32 = 800;
    pub const INV_ROTZ: u32 = 804;
    pub const INV_ANIMATE: u32 = 808;
}

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
        member: "nameid",
        element: 0,
        offset: offset::NAMEID,
        count: 1,
    },
    FieldSlot {
        member: "hp",
        element: 0,
        offset: offset::HP,
        count: 1,
    },
    FieldSlot {
        member: "hp_max",
        element: 0,
        offset: offset::HP_MAX,
        count: 1,
    },
    FieldSlot {
        member: "mainflag",
        element: 0,
        offset: offset::MAINFLAG,
        count: 1,
    },
    FieldSlot {
        member: "flags",
        element: 0,
        offset: offset::FLAGS,
        count: 1,
    },
    FieldSlot {
        member: "weight",
        element: 0,
        offset: offset::WEIGHT,
        count: 1,
    },
    FieldSlot {
        member: "value",
        element: 0,
        offset: offset::VALUE,
        count: 1,
    },
    FieldSlot {
        member: "damagetype",
        element: 0,
        offset: offset::DAMAGETYPE,
        count: 1,
    },
    FieldSlot {
        member: "damagetotal",
        element: 0,
        offset: offset::DAMAGE_TOTAL,
        count: 1,
    },
    FieldSlot {
        member: "damage",
        element: 0,
        offset: offset::DAMAGE,
        count: 8,
    },
    FieldSlot {
        member: "wear",
        element: 0,
        offset: offset::WEAR,
        count: 1,
    },
    FieldSlot {
        member: "protection",
        element: 0,
        offset: offset::PROTECTION,
        count: 8,
    },
    FieldSlot {
        member: "nutrition",
        element: 0,
        offset: offset::NUTRITION,
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
        member: "change_atr",
        element: 0,
        offset: offset::CHANGE_ATR,
        count: 3,
    },
    FieldSlot {
        member: "change_value",
        element: 0,
        offset: offset::CHANGE_VALUE,
        count: 3,
    },
    FieldSlot {
        member: "magic",
        element: 0,
        offset: offset::MAGIC,
        count: 1,
    },
    FieldSlot {
        member: "on_equip",
        element: 0,
        offset: offset::ON_EQUIP,
        count: 1,
    },
    FieldSlot {
        member: "on_unequip",
        element: 0,
        offset: offset::ON_UNEQUIP,
        count: 1,
    },
    FieldSlot {
        member: "on_state",
        element: 0,
        offset: offset::ON_STATE,
        count: 4,
    },
    FieldSlot {
        member: "owner",
        element: 0,
        offset: offset::OWNER,
        count: 1,
    },
    FieldSlot {
        member: "ownerguild",
        element: 0,
        offset: offset::OWNERGUILD,
        count: 1,
    },
    FieldSlot {
        member: "disguiseguild",
        element: 0,
        offset: offset::DISGUISEGUILD,
        count: 1,
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
    FieldSlot {
        member: "effect",
        element: 0,
        offset: offset::EFFECT,
        count: 1,
    },
    FieldSlot {
        member: "visual_skin",
        element: 0,
        offset: offset::VISUAL_SKIN,
        count: 1,
    },
    FieldSlot {
        member: "scemename",
        element: 0,
        offset: offset::SCEMENAME,
        count: 1,
    },
    FieldSlot {
        member: "material",
        element: 0,
        offset: offset::MATERIAL,
        count: 1,
    },
    FieldSlot {
        member: "munition",
        element: 0,
        offset: offset::MUNITION,
        count: 1,
    },
    FieldSlot {
        member: "spell",
        element: 0,
        offset: offset::SPELL,
        count: 1,
    },
    FieldSlot {
        member: "range",
        element: 0,
        offset: offset::RANGE,
        count: 1,
    },
    FieldSlot {
        member: "mag_circle",
        element: 0,
        offset: offset::MAG_CIRCLE,
        count: 1,
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
        member: "inv_zbias",
        element: 0,
        offset: offset::INV_ZBIAS,
        count: 1,
    },
    FieldSlot {
        member: "inv_rotx",
        element: 0,
        offset: offset::INV_ROTX,
        count: 1,
    },
    FieldSlot {
        member: "inv_roty",
        element: 0,
        offset: offset::INV_ROTY,
        count: 1,
    },
    FieldSlot {
        member: "inv_rotz",
        element: 0,
        offset: offset::INV_ROTZ,
        count: 1,
    },
    FieldSlot {
        member: "inv_animate",
        element: 0,
        offset: offset::INV_ANIMATE,
        count: 1,
    },
];

impl ScriptVmAccess for Item {
    fn read(&self, offset: u32, element: u32) -> Option<i32> {
        let index = element as usize;
        match (offset, element) {
            (offset::ID, 0) => Some(self.id),
            (offset::NAME, 0) => Some(self.name.map_or(0, StringId::value)),
            (offset::NAMEID, 0) => Some(self.nameid.map_or(0, StringId::value)),
            (offset::HP, 0) => Some(self.hp),
            (offset::HP_MAX, 0) => Some(self.hp_max),
            (offset::MAINFLAG, 0) => Some(self.mainflag),
            (offset::FLAGS, 0) => Some(self.flags),
            (offset::WEIGHT, 0) => Some(self.weight),
            (offset::VALUE, 0) => Some(self.value),
            (offset::DAMAGETYPE, 0) => Some(self.damagetype),
            (offset::DAMAGE_TOTAL, 0) => Some(self.damage_total),
            (offset::DAMAGE, _) => self.damage.get(index).copied(),
            (offset::WEAR, 0) => Some(self.wear),
            (offset::PROTECTION, _) => self.protection.get(index).copied(),
            (offset::NUTRITION, 0) => Some(self.nutrition),
            (offset::COND_ATR, _) => self.cond_atr.get(index).copied(),
            (offset::COND_VALUE, _) => self.cond_value.get(index).copied(),
            (offset::CHANGE_ATR, _) => self.change_atr.get(index).copied(),
            (offset::CHANGE_VALUE, _) => self.change_value.get(index).copied(),
            (offset::MAGIC, 0) => Some(self.magic.map_or(0, FunctionId::value)),
            (offset::ON_EQUIP, 0) => Some(self.on_equip.map_or(0, FunctionId::value)),
            (offset::ON_UNEQUIP, 0) => Some(self.on_unequip.map_or(0, FunctionId::value)),
            (offset::ON_STATE, _) => self
                .on_state
                .get(index)
                .map(|id| id.map_or(0, FunctionId::value)),
            (offset::OWNER, 0) => Some(self.owner.map_or(0, FunctionId::value)),
            (offset::OWNERGUILD, 0) => Some(self.ownerguild),
            (offset::DISGUISEGUILD, 0) => Some(self.disguiseguild),
            (offset::VISUAL, 0) => Some(self.visual.map_or(0, StringId::value)),
            (offset::VISUAL_CHANGE, 0) => Some(self.visual_change.map_or(0, StringId::value)),
            (offset::EFFECT, 0) => Some(self.effect.map_or(0, StringId::value)),
            (offset::VISUAL_SKIN, 0) => Some(self.visual_skin),
            (offset::SCEMENAME, 0) => Some(self.scemename.map_or(0, StringId::value)),
            (offset::MATERIAL, 0) => Some(self.material),
            (offset::MUNITION, 0) => Some(self.munition),
            (offset::SPELL, 0) => Some(self.spell),
            (offset::RANGE, 0) => Some(self.range),
            (offset::MAG_CIRCLE, 0) => Some(self.mag_circle),
            (offset::DESCRIPTION, 0) => Some(self.description.map_or(0, StringId::value)),
            (offset::TEXT, _) => self.text.get(index).map(|id| id.map_or(0, StringId::value)),
            (offset::COUNT, _) => self.count.get(index).copied(),
            (offset::INV_ZBIAS, 0) => Some(self.inv_zbias),
            (offset::INV_ROTX, 0) => Some(self.inv_rotx),
            (offset::INV_ROTY, 0) => Some(self.inv_roty),
            (offset::INV_ROTZ, 0) => Some(self.inv_rotz),
            (offset::INV_ANIMATE, 0) => Some(self.inv_animate),
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
            (offset::NAMEID, 0) => {
                self.nameid = StringId::new(value);
                true
            }
            (offset::HP, 0) => {
                self.hp = value;
                true
            }
            (offset::HP_MAX, 0) => {
                self.hp_max = value;
                true
            }
            (offset::MAINFLAG, 0) => {
                self.mainflag = value;
                true
            }
            (offset::FLAGS, 0) => {
                self.flags = value;
                true
            }
            (offset::WEIGHT, 0) => {
                self.weight = value;
                true
            }
            (offset::VALUE, 0) => {
                self.value = value;
                true
            }
            (offset::DAMAGETYPE, 0) => {
                self.damagetype = value;
                true
            }
            (offset::DAMAGE_TOTAL, 0) => {
                self.damage_total = value;
                true
            }
            (offset::DAMAGE, _) => write_slot(&mut self.damage, index, value),
            (offset::WEAR, 0) => {
                self.wear = value;
                true
            }
            (offset::PROTECTION, _) => write_slot(&mut self.protection, index, value),
            (offset::NUTRITION, 0) => {
                self.nutrition = value;
                true
            }
            (offset::COND_ATR, _) => write_slot(&mut self.cond_atr, index, value),
            (offset::COND_VALUE, _) => write_slot(&mut self.cond_value, index, value),
            (offset::CHANGE_ATR, _) => write_slot(&mut self.change_atr, index, value),
            (offset::CHANGE_VALUE, _) => write_slot(&mut self.change_value, index, value),
            (offset::MAGIC, 0) => {
                self.magic = function_id(value);
                true
            }
            (offset::ON_EQUIP, 0) => {
                self.on_equip = function_id(value);
                true
            }
            (offset::ON_UNEQUIP, 0) => {
                self.on_unequip = function_id(value);
                true
            }
            (offset::ON_STATE, _) => write_function_slot(&mut self.on_state, index, value),
            (offset::OWNER, 0) => {
                self.owner = function_id(value);
                true
            }
            (offset::OWNERGUILD, 0) => {
                self.ownerguild = value;
                true
            }
            (offset::DISGUISEGUILD, 0) => {
                self.disguiseguild = value;
                true
            }
            (offset::VISUAL, 0) => {
                self.visual = StringId::new(value);
                true
            }
            (offset::VISUAL_CHANGE, 0) => {
                self.visual_change = StringId::new(value);
                true
            }
            (offset::EFFECT, 0) => {
                self.effect = StringId::new(value);
                true
            }
            (offset::VISUAL_SKIN, 0) => {
                self.visual_skin = value;
                true
            }
            (offset::SCEMENAME, 0) => {
                self.scemename = StringId::new(value);
                true
            }
            (offset::MATERIAL, 0) => {
                self.material = value;
                true
            }
            (offset::MUNITION, 0) => {
                self.munition = value;
                true
            }
            (offset::SPELL, 0) => {
                self.spell = value;
                true
            }
            (offset::RANGE, 0) => {
                self.range = value;
                true
            }
            (offset::MAG_CIRCLE, 0) => {
                self.mag_circle = value;
                true
            }
            (offset::DESCRIPTION, 0) => {
                self.description = StringId::new(value);
                true
            }
            (offset::TEXT, _) => write_string_slot(&mut self.text, index, value),
            (offset::COUNT, _) => write_slot(&mut self.count, index, value),
            (offset::INV_ZBIAS, 0) => {
                self.inv_zbias = value;
                true
            }
            (offset::INV_ROTX, 0) => {
                self.inv_rotx = value;
                true
            }
            (offset::INV_ROTY, 0) => {
                self.inv_roty = value;
                true
            }
            (offset::INV_ROTZ, 0) => {
                self.inv_rotz = value;
                true
            }
            (offset::INV_ANIMATE, 0) => {
                self.inv_animate = value;
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

fn write_string_slot(slots: &mut [Option<StringId>], index: usize, value: i32) -> bool {
    let Some(slot) = slots.get_mut(index) else {
        return false;
    };
    *slot = StringId::new(value);
    true
}
