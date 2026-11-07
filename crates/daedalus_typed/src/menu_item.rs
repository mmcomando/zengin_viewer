//! Semantic model of the MENU.DAT `C_MenuItem` members the host uses.

use super::access::{ScriptConversion, ScriptVmAccess};
use super::ids::{FunctionId, StringId};
use super::registry::FieldSlot;

/// Kind of a menu item (`C_MenuItem.type`), unknown values are preserved.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum MenuItemKind {
    /// Undefined item type.
    #[default]
    Undefined,
    /// Plain text line.
    Text,
    /// Slider.
    Slider,
    /// Text input field.
    Input,
    /// Mouse cursor.
    Cursor,
    /// Choice box.
    Choicebox,
    /// Button.
    Button,
    /// List box.
    Listbox,
    /// A value not known to the host, preserved as written by the script.
    Other(i32),
}

impl ScriptConversion for MenuItemKind {
    fn from_i32(value: i32) -> Self {
        match value {
            0 => Self::Undefined,
            1 => Self::Text,
            2 => Self::Slider,
            3 => Self::Input,
            4 => Self::Cursor,
            5 => Self::Choicebox,
            6 => Self::Button,
            7 => Self::Listbox,
            other => Self::Other(other),
        }
    }

    fn to_i32(self) -> i32 {
        match self {
            Self::Undefined => 0,
            Self::Text => 1,
            Self::Slider => 2,
            Self::Input => 3,
            Self::Cursor => 4,
            Self::Choicebox => 5,
            Self::Button => 6,
            Self::Listbox => 7,
            Self::Other(value) => value,
        }
    }
}

/// `C_MenuItem` members modeled by the host.
#[derive(Debug, Default, Clone)]
pub struct MenuItem {
    /// Font name string id.
    pub font_name: Option<StringId>,
    /// Text string ids.
    pub text: [Option<StringId>; 10],
    /// Background picture string id.
    pub back_pic: Option<StringId>,
    /// Alpha mode string id.
    pub alpha_mode: Option<StringId>,
    /// Alpha value.
    pub alpha: i32,
    /// Item kind.
    pub kind: MenuItemKind,
    /// Selection action ids.
    pub on_sel_action: [i32; 5],
    /// Selection action text string ids.
    pub on_sel_action_s: [Option<StringId>; 5],
    /// Set option string id.
    pub on_chg_set_option: Option<StringId>,
    /// Set option section string id.
    pub on_chg_set_option_section: Option<StringId>,
    /// Event action function ids.
    pub on_event_action: [Option<FunctionId>; 10],
    /// Position on screen.
    pub pos_x: i32,
    /// Position on screen.
    pub pos_y: i32,
    /// Size on screen.
    pub dim_x: i32,
    /// Size on screen.
    pub dim_y: i32,
    /// List box inner frame size.
    pub frame_size_x: i32,
    /// List box inner frame size.
    pub frame_size_y: i32,
    /// Item flags.
    pub item_flags: i32,
}

mod offset {
    pub const FONT_NAME: u32 = 0;
    pub const TEXT: u32 = 20;
    pub const BACK_PIC: u32 = 220;
    pub const ALPHA_MODE: u32 = 240;
    pub const ALPHA: u32 = 260;
    pub const KIND: u32 = 264;
    pub const ON_SEL_ACTION: u32 = 268;
    pub const ON_SEL_ACTION_S: u32 = 288;
    pub const ON_CHG_SET_OPTION: u32 = 388;
    pub const ON_CHG_SET_OPTION_SECTION: u32 = 408;
    pub const ON_EVENT_ACTION: u32 = 428;
    pub const POS_X: u32 = 468;
    pub const POS_Y: u32 = 472;
    pub const DIM_X: u32 = 476;
    pub const DIM_Y: u32 = 480;
    pub const ITEM_FLAGS: u32 = 488;
    pub const FRAME_SIZE_X: u32 = 596;
    pub const FRAME_SIZE_Y: u32 = 600;
}

pub(crate) const FIELD_SLOTS: &[FieldSlot] = &[
    FieldSlot {
        member: "fontname",
        element: 0,
        offset: offset::FONT_NAME,
        count: 1,
    },
    FieldSlot {
        member: "text",
        element: 0,
        offset: offset::TEXT,
        count: 10,
    },
    FieldSlot {
        member: "backpic",
        element: 0,
        offset: offset::BACK_PIC,
        count: 1,
    },
    FieldSlot {
        member: "alphamode",
        element: 0,
        offset: offset::ALPHA_MODE,
        count: 1,
    },
    FieldSlot {
        member: "alpha",
        element: 0,
        offset: offset::ALPHA,
        count: 1,
    },
    FieldSlot {
        member: "type",
        element: 0,
        offset: offset::KIND,
        count: 1,
    },
    FieldSlot {
        member: "onselaction",
        element: 0,
        offset: offset::ON_SEL_ACTION,
        count: 5,
    },
    FieldSlot {
        member: "onselaction_s",
        element: 0,
        offset: offset::ON_SEL_ACTION_S,
        count: 5,
    },
    FieldSlot {
        member: "onchgsetoption",
        element: 0,
        offset: offset::ON_CHG_SET_OPTION,
        count: 1,
    },
    FieldSlot {
        member: "onchgsetoptionsection",
        element: 0,
        offset: offset::ON_CHG_SET_OPTION_SECTION,
        count: 1,
    },
    FieldSlot {
        member: "oneventaction",
        element: 0,
        offset: offset::ON_EVENT_ACTION,
        count: 10,
    },
    FieldSlot {
        member: "posx",
        element: 0,
        offset: offset::POS_X,
        count: 1,
    },
    FieldSlot {
        member: "posy",
        element: 0,
        offset: offset::POS_Y,
        count: 1,
    },
    FieldSlot {
        member: "dimx",
        element: 0,
        offset: offset::DIM_X,
        count: 1,
    },
    FieldSlot {
        member: "dimy",
        element: 0,
        offset: offset::DIM_Y,
        count: 1,
    },
    FieldSlot {
        member: "flags",
        element: 0,
        offset: offset::ITEM_FLAGS,
        count: 1,
    },
    FieldSlot {
        member: "framesizex",
        element: 0,
        offset: offset::FRAME_SIZE_X,
        count: 1,
    },
    FieldSlot {
        member: "framesizey",
        element: 0,
        offset: offset::FRAME_SIZE_Y,
        count: 1,
    },
];

impl ScriptVmAccess for MenuItem {
    fn read(&self, offset: u32, element: u32) -> Option<i32> {
        let index = element as usize;
        match (offset, element) {
            (offset::FONT_NAME, 0) => Some(self.font_name.map_or(0, StringId::value)),
            (offset::TEXT, _) => self.text.get(index).map(|id| id.map_or(0, StringId::value)),
            (offset::BACK_PIC, 0) => Some(self.back_pic.map_or(0, StringId::value)),
            (offset::ALPHA_MODE, 0) => Some(self.alpha_mode.map_or(0, StringId::value)),
            (offset::ALPHA, 0) => Some(self.alpha),
            (offset::KIND, 0) => Some(self.kind.to_i32()),
            (offset::ON_SEL_ACTION, _) => self.on_sel_action.get(index).copied(),
            (offset::ON_SEL_ACTION_S, _) => self
                .on_sel_action_s
                .get(index)
                .map(|id| id.map_or(0, StringId::value)),
            (offset::ON_CHG_SET_OPTION, 0) => {
                Some(self.on_chg_set_option.map_or(0, StringId::value))
            }
            (offset::ON_CHG_SET_OPTION_SECTION, 0) => {
                Some(self.on_chg_set_option_section.map_or(0, StringId::value))
            }
            (offset::ON_EVENT_ACTION, _) => self
                .on_event_action
                .get(index)
                .map(|id| id.map_or(0, FunctionId::value)),
            (offset::POS_X, 0) => Some(self.pos_x),
            (offset::POS_Y, 0) => Some(self.pos_y),
            (offset::DIM_X, 0) => Some(self.dim_x),
            (offset::DIM_Y, 0) => Some(self.dim_y),
            (offset::ITEM_FLAGS, 0) => Some(self.item_flags),
            (offset::FRAME_SIZE_X, 0) => Some(self.frame_size_x),
            (offset::FRAME_SIZE_Y, 0) => Some(self.frame_size_y),
            _ => None,
        }
    }

    fn write(&mut self, offset: u32, element: u32, value: i32) -> bool {
        let index = element as usize;
        match (offset, element) {
            (offset::FONT_NAME, 0) => {
                self.font_name = StringId::new(value);
                true
            }
            (offset::TEXT, _) => write_string_slot(&mut self.text, index, value),
            (offset::BACK_PIC, 0) => {
                self.back_pic = StringId::new(value);
                true
            }
            (offset::ALPHA_MODE, 0) => {
                self.alpha_mode = StringId::new(value);
                true
            }
            (offset::ALPHA, 0) => {
                self.alpha = value;
                true
            }
            (offset::KIND, 0) => {
                self.kind = MenuItemKind::from_i32(value);
                true
            }
            (offset::ON_SEL_ACTION, _) => {
                let Some(slot) = self.on_sel_action.get_mut(index) else {
                    return false;
                };
                *slot = value;
                true
            }
            (offset::ON_SEL_ACTION_S, _) => {
                write_string_slot(&mut self.on_sel_action_s, index, value)
            }
            (offset::ON_CHG_SET_OPTION, 0) => {
                self.on_chg_set_option = StringId::new(value);
                true
            }
            (offset::ON_CHG_SET_OPTION_SECTION, 0) => {
                self.on_chg_set_option_section = StringId::new(value);
                true
            }
            (offset::ON_EVENT_ACTION, _) => {
                let Some(slot) = self.on_event_action.get_mut(index) else {
                    return false;
                };
                *slot = (value > 1).then(|| FunctionId::new(value)).flatten();
                true
            }
            (offset::POS_X, 0) => {
                self.pos_x = value;
                true
            }
            (offset::POS_Y, 0) => {
                self.pos_y = value;
                true
            }
            (offset::DIM_X, 0) => {
                self.dim_x = value;
                true
            }
            (offset::DIM_Y, 0) => {
                self.dim_y = value;
                true
            }
            (offset::ITEM_FLAGS, 0) => {
                self.item_flags = value;
                true
            }
            (offset::FRAME_SIZE_X, 0) => {
                self.frame_size_x = value;
                true
            }
            (offset::FRAME_SIZE_Y, 0) => {
                self.frame_size_y = value;
                true
            }
            _ => false,
        }
    }
}

fn write_string_slot(slots: &mut [Option<StringId>], index: usize, value: i32) -> bool {
    let Some(slot) = slots.get_mut(index) else {
        return false;
    };
    *slot = StringId::new(value);
    true
}
