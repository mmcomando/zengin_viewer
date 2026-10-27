//! Semantic model of the MENU.DAT `C_Menu` members the host uses.

use super::access::ScriptVmAccess;
use super::ids::StringId;
use super::registry::FieldSlot;

/// Number of `C_Menu.items` entries in G2.
pub const MENU_ITEMS_MAX: usize = 150;

/// `C_Menu` members modeled by the host.
#[derive(Debug, Clone)]
pub struct Menu {
    /// Background picture string id.
    pub back_pic: Option<StringId>,
    /// Background world string id.
    pub back_world: Option<StringId>,
    /// Position on screen.
    pub pos_x: i32,
    /// Position on screen.
    pub pos_y: i32,
    /// Size on screen.
    pub dim_x: i32,
    /// Size on screen.
    pub dim_y: i32,
    /// Alpha value.
    pub alpha: i32,
    /// Music theme string id.
    pub music_theme: Option<StringId>,
    /// Event timer in milliseconds.
    pub event_timer_msec: i32,
    /// Item instance name string ids, indexed by slot.
    pub items: [Option<StringId>; MENU_ITEMS_MAX],
    /// Menu flags.
    pub flags: i32,
    /// Default item outside the game.
    pub default_out_game: i32,
    /// Default item inside the game.
    pub default_in_game: i32,
}

impl Default for Menu {
    fn default() -> Self {
        Self {
            back_pic: None,
            back_world: None,
            pos_x: 0,
            pos_y: 0,
            dim_x: 0,
            dim_y: 0,
            alpha: 0,
            music_theme: None,
            event_timer_msec: 0,
            items: [None; MENU_ITEMS_MAX],
            flags: 0,
            default_out_game: 0,
            default_in_game: 0,
        }
    }
}

mod offset {
    pub const BACK_PIC: u32 = 0;
    pub const BACK_WORLD: u32 = 20;
    pub const POS_X: u32 = 40;
    pub const POS_Y: u32 = 44;
    pub const DIM_X: u32 = 48;
    pub const DIM_Y: u32 = 52;
    pub const ALPHA: u32 = 56;
    pub const MUSIC_THEME: u32 = 60;
    pub const EVENT_TIMER_MSEC: u32 = 80;
    pub const ITEMS: u32 = 84;
    pub const FLAGS: u32 = 3084;
    pub const DEFAULT_OUT_GAME: u32 = 3088;
    pub const DEFAULT_IN_GAME: u32 = 3092;
}

pub(crate) const FIELD_SLOTS: &[FieldSlot] = &[
    FieldSlot {
        member: "backpic",
        element: 0,
        offset: offset::BACK_PIC,
        count: 1,
    },
    FieldSlot {
        member: "backworld",
        element: 0,
        offset: offset::BACK_WORLD,
        count: 1,
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
        member: "alpha",
        element: 0,
        offset: offset::ALPHA,
        count: 1,
    },
    FieldSlot {
        member: "musictheme",
        element: 0,
        offset: offset::MUSIC_THEME,
        count: 1,
    },
    FieldSlot {
        member: "eventtimermsec",
        element: 0,
        offset: offset::EVENT_TIMER_MSEC,
        count: 1,
    },
    FieldSlot {
        member: "items",
        element: 0,
        offset: offset::ITEMS,
        count: MENU_ITEMS_MAX as u32,
    },
    FieldSlot {
        member: "flags",
        element: 0,
        offset: offset::FLAGS,
        count: 1,
    },
    FieldSlot {
        member: "defaultoutgame",
        element: 0,
        offset: offset::DEFAULT_OUT_GAME,
        count: 1,
    },
    FieldSlot {
        member: "defaultingame",
        element: 0,
        offset: offset::DEFAULT_IN_GAME,
        count: 1,
    },
];

impl ScriptVmAccess for Menu {
    fn read(&self, offset: u32, element: u32) -> Option<i32> {
        let index = element as usize;
        match (offset, element) {
            (offset::BACK_PIC, 0) => Some(self.back_pic.map_or(0, StringId::value)),
            (offset::BACK_WORLD, 0) => Some(self.back_world.map_or(0, StringId::value)),
            (offset::POS_X, 0) => Some(self.pos_x),
            (offset::POS_Y, 0) => Some(self.pos_y),
            (offset::DIM_X, 0) => Some(self.dim_x),
            (offset::DIM_Y, 0) => Some(self.dim_y),
            (offset::ALPHA, 0) => Some(self.alpha),
            (offset::MUSIC_THEME, 0) => Some(self.music_theme.map_or(0, StringId::value)),
            (offset::EVENT_TIMER_MSEC, 0) => Some(self.event_timer_msec),
            (offset::ITEMS, _) => self
                .items
                .get(index)
                .map(|id| id.map_or(0, StringId::value)),
            (offset::FLAGS, 0) => Some(self.flags),
            (offset::DEFAULT_OUT_GAME, 0) => Some(self.default_out_game),
            (offset::DEFAULT_IN_GAME, 0) => Some(self.default_in_game),
            _ => None,
        }
    }

    fn write(&mut self, offset: u32, element: u32, value: i32) -> bool {
        let index = element as usize;
        match (offset, element) {
            (offset::BACK_PIC, 0) => {
                self.back_pic = StringId::new(value);
                true
            }
            (offset::BACK_WORLD, 0) => {
                self.back_world = StringId::new(value);
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
            (offset::ALPHA, 0) => {
                self.alpha = value;
                true
            }
            (offset::MUSIC_THEME, 0) => {
                self.music_theme = StringId::new(value);
                true
            }
            (offset::EVENT_TIMER_MSEC, 0) => {
                self.event_timer_msec = value;
                true
            }
            (offset::ITEMS, _) => {
                let Some(slot) = self.items.get_mut(index) else {
                    return false;
                };
                *slot = StringId::new(value);
                true
            }
            (offset::FLAGS, 0) => {
                self.flags = value;
                true
            }
            (offset::DEFAULT_OUT_GAME, 0) => {
                self.default_out_game = value;
                true
            }
            (offset::DEFAULT_IN_GAME, 0) => {
                self.default_in_game = value;
                true
            }
            _ => false,
        }
    }
}
