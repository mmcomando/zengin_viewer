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

/// Undefined item type.
pub const MENU_ITEM_UNDEF: i32 = 0;
/// Plain text line, see [`MenuItemKind::Text`].
pub const MENU_ITEM_TEXT: i32 = 1;
/// Slider, see [`MenuItemKind::Slider`].
pub const MENU_ITEM_SLIDER: i32 = 2;
/// Text input field, see [`MenuItemKind::Input`].
pub const MENU_ITEM_INPUT: i32 = 3;
/// Mouse cursor, see [`MenuItemKind::Cursor`].
pub const MENU_ITEM_CURSOR: i32 = 4;
/// Choice box, see [`MenuItemKind::Choicebox`].
pub const MENU_ITEM_CHOICEBOX: i32 = 5;
/// Button, see [`MenuItemKind::Button`].
pub const MENU_ITEM_BUTTON: i32 = 6;
/// List box, see [`MenuItemKind::Listbox`].
pub const MENU_ITEM_LISTBOX: i32 = 7;

/// `IT_CHROMAKEYED` item flag.
pub const MENU_ITEM_FLAG_CHROMAKEYED: i32 = 1;
/// `IT_TRANSPARENT` item flag.
pub const MENU_ITEM_FLAG_TRANSPARENT: i32 = 2;
/// `IT_SELECTABLE` item flag.
pub const MENU_ITEM_FLAG_SELECTABLE: i32 = 4;
/// `IT_MOVEABLE` item flag.
pub const MENU_ITEM_FLAG_MOVEABLE: i32 = 8;
/// `IT_TXT_CENTER` item flag: item text is centered.
pub const MENU_ITEM_FLAG_TXT_CENTER: i32 = 16;
/// `IT_DISABLED` item flag.
pub const MENU_ITEM_FLAG_DISABLED: i32 = 32;
/// `IT_FADE` item flag.
pub const MENU_ITEM_FLAG_FADE: i32 = 64;
/// `IT_EFFECTS_NEXT` item flag.
pub const MENU_ITEM_FLAG_EFFECTS_NEXT: i32 = 128;
/// `IT_ONLY_OUT_GAME` item flag: item is hidden inside the game.
pub const MENU_ITEM_FLAG_ONLY_OUT_GAME: i32 = 256;
/// `IT_ONLY_IN_GAME` item flag: item is hidden outside the game.
pub const MENU_ITEM_FLAG_ONLY_IN_GAME: i32 = 512;
/// `IT_PERF_OPTION` item flag.
pub const MENU_ITEM_FLAG_PERF_OPTION: i32 = 1 << 10;
/// `IT_MULTILINE` item flag.
pub const MENU_ITEM_FLAG_MULTILINE: i32 = 1 << 11;
/// `IT_NEEDS_APPLY` item flag.
pub const MENU_ITEM_FLAG_NEEDS_APPLY: i32 = 1 << 12;
/// `IT_NEEDS_RESTART` item flag.
pub const MENU_ITEM_FLAG_NEEDS_RESTART: i32 = 1 << 13;
/// `IT_EXTENDED_MENU` item flag.
pub const MENU_ITEM_FLAG_EXTENDED_MENU: i32 = 1 << 14;

/// No select action.
pub const SEL_ACTION_UNDEF: i32 = 0;
/// Returns to the previous menu.
pub const SEL_ACTION_BACK: i32 = 1;
/// Opens another menu.
pub const SEL_ACTION_START_MENU: i32 = 2;
/// Starts an item action.
pub const SEL_ACTION_START_ITEM: i32 = 3;
/// Closes the menu.
pub const SEL_ACTION_CLOSE: i32 = 4;
/// Runs console commands.
pub const SEL_ACTION_CON_COMMANDS: i32 = 5;
/// Plays a sound.
pub const SEL_ACTION_PLAY_SOUND: i32 = 6;
/// Executes script commands.
pub const SEL_ACTION_EXEC_COMMANDS: i32 = 7;

/// Index of the `execute` event in `C_MenuItem.onEventAction`.
pub const MENU_EVENT_EXECUTE: usize = 1;

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
    /// Scale the item starts at when it opens.
    pub size_start_scale: f32,
    /// Item flags.
    pub item_flags: i32,
    /// Delay before the open animation starts.
    pub open_delay_time: f32,
    /// Duration of the open animation.
    pub open_duration: f32,
    /// Free float values for the item script.
    pub user_float: [f32; 4],
    /// Free string ids for the item script.
    pub user_string: [Option<StringId>; 4],
    /// List box inner frame size.
    pub frame_size_x: i32,
    /// List box inner frame size.
    pub frame_size_y: i32,
    /// Option section string id that hides this item.
    pub hide_if_option_section_set: Option<StringId>,
    /// Option string id that hides this item.
    pub hide_if_option_set: Option<StringId>,
    /// Value that hides this item.
    pub hide_on_value: i32,
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
    pub const SIZE_START_SCALE: u32 = 484;
    pub const ITEM_FLAGS: u32 = 488;
    pub const OPEN_DELAY_TIME: u32 = 492;
    pub const OPEN_DURATION: u32 = 496;
    pub const USER_FLOAT: u32 = 500;
    pub const USER_STRING: u32 = 516;
    pub const FRAME_SIZE_X: u32 = 596;
    pub const FRAME_SIZE_Y: u32 = 600;
    pub const HIDE_IF_OPTION_SECTION_SET: u32 = 604;
    pub const HIDE_IF_OPTION_SET: u32 = 624;
    pub const HIDE_ON_VALUE: u32 = 644;
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
        member: "sizestartscale",
        element: 0,
        offset: offset::SIZE_START_SCALE,
        count: 1,
    },
    FieldSlot {
        member: "flags",
        element: 0,
        offset: offset::ITEM_FLAGS,
        count: 1,
    },
    FieldSlot {
        member: "opendelaytime",
        element: 0,
        offset: offset::OPEN_DELAY_TIME,
        count: 1,
    },
    FieldSlot {
        member: "openduration",
        element: 0,
        offset: offset::OPEN_DURATION,
        count: 1,
    },
    FieldSlot {
        member: "userfloat",
        element: 0,
        offset: offset::USER_FLOAT,
        count: 4,
    },
    FieldSlot {
        member: "userstring",
        element: 0,
        offset: offset::USER_STRING,
        count: 4,
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
    FieldSlot {
        member: "hideifoptionsectionset",
        element: 0,
        offset: offset::HIDE_IF_OPTION_SECTION_SET,
        count: 1,
    },
    FieldSlot {
        member: "hideifoptionset",
        element: 0,
        offset: offset::HIDE_IF_OPTION_SET,
        count: 1,
    },
    FieldSlot {
        member: "hideonvalue",
        element: 0,
        offset: offset::HIDE_ON_VALUE,
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
            (offset::SIZE_START_SCALE, 0) => Some(self.size_start_scale.to_bits() as i32),
            (offset::ITEM_FLAGS, 0) => Some(self.item_flags),
            (offset::OPEN_DELAY_TIME, 0) => Some(self.open_delay_time.to_bits() as i32),
            (offset::OPEN_DURATION, 0) => Some(self.open_duration.to_bits() as i32),
            (offset::USER_FLOAT, _) => self.user_float.get(index).map(|v| v.to_bits() as i32),
            (offset::USER_STRING, _) => self
                .user_string
                .get(index)
                .map(|id| id.map_or(0, StringId::value)),
            (offset::FRAME_SIZE_X, 0) => Some(self.frame_size_x),
            (offset::FRAME_SIZE_Y, 0) => Some(self.frame_size_y),
            (offset::HIDE_IF_OPTION_SECTION_SET, 0) => {
                Some(self.hide_if_option_section_set.map_or(0, StringId::value))
            }
            (offset::HIDE_IF_OPTION_SET, 0) => {
                Some(self.hide_if_option_set.map_or(0, StringId::value))
            }
            (offset::HIDE_ON_VALUE, 0) => Some(self.hide_on_value),
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
            (offset::SIZE_START_SCALE, 0) => {
                self.size_start_scale = f32::from_bits(value as u32);
                true
            }
            (offset::ITEM_FLAGS, 0) => {
                self.item_flags = value;
                true
            }
            (offset::OPEN_DELAY_TIME, 0) => {
                self.open_delay_time = f32::from_bits(value as u32);
                true
            }
            (offset::OPEN_DURATION, 0) => {
                self.open_duration = f32::from_bits(value as u32);
                true
            }
            (offset::USER_FLOAT, _) => {
                let Some(slot) = self.user_float.get_mut(index) else {
                    return false;
                };
                *slot = f32::from_bits(value as u32);
                true
            }
            (offset::USER_STRING, _) => write_string_slot(&mut self.user_string, index, value),
            (offset::FRAME_SIZE_X, 0) => {
                self.frame_size_x = value;
                true
            }
            (offset::FRAME_SIZE_Y, 0) => {
                self.frame_size_y = value;
                true
            }
            (offset::HIDE_IF_OPTION_SECTION_SET, 0) => {
                self.hide_if_option_section_set = StringId::new(value);
                true
            }
            (offset::HIDE_IF_OPTION_SET, 0) => {
                self.hide_if_option_set = StringId::new(value);
                true
            }
            (offset::HIDE_ON_VALUE, 0) => {
                self.hide_on_value = value;
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
