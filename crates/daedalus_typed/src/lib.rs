#![warn(missing_docs)]
//! Semantic Rust models of the G2 daedalus script classes the host uses.
//!
//! Each class maps raw `(byte offset, element)` accesses from converted script
//! code to named, typed Rust fields. Only fields the host or the scripts
//! actually use are modeled; primitives and unmodeled classes use generic word
//! storage in [`TypedRegistry`].
//!
//! The hardcoded G2 offsets are validated against the loaded script at registry
//! build time. A mismatch panics, because the compiled accessors would otherwise
//! silently read and write the wrong fields.

mod access;
mod generic;
pub mod ids;
mod info;
mod item;
mod menu;
mod menu_item;
mod npc;
mod registry;
mod symbol;

pub use access::{ScriptConversion, ScriptVmAccess};
pub use generic::GenericVariable;
pub use info::Info;
pub use item::Item;
pub use menu::{
    MENU_FLAG_ALIGN_CENTER, MENU_FLAG_DONT_SCALE_DIM, MENU_FLAG_DONT_SCALE_POS,
    MENU_FLAG_EXCLUSIVE, MENU_FLAG_NO_ANI, MENU_FLAG_OVERTOP, MENU_FLAG_SHOW_INFO, MENU_SCRIPT_DIV,
    Menu,
};
pub use menu_item::{
    MENU_EVENT_EXECUTE, MENU_ITEM_BUTTON, MENU_ITEM_CHOICEBOX, MENU_ITEM_CURSOR,
    MENU_ITEM_FLAG_CHROMAKEYED, MENU_ITEM_FLAG_DISABLED, MENU_ITEM_FLAG_EFFECTS_NEXT,
    MENU_ITEM_FLAG_EXTENDED_MENU, MENU_ITEM_FLAG_FADE, MENU_ITEM_FLAG_MOVEABLE,
    MENU_ITEM_FLAG_MULTILINE, MENU_ITEM_FLAG_NEEDS_APPLY, MENU_ITEM_FLAG_NEEDS_RESTART,
    MENU_ITEM_FLAG_ONLY_IN_GAME, MENU_ITEM_FLAG_ONLY_OUT_GAME, MENU_ITEM_FLAG_PERF_OPTION,
    MENU_ITEM_FLAG_SELECTABLE, MENU_ITEM_FLAG_TRANSPARENT, MENU_ITEM_FLAG_TXT_CENTER,
    MENU_ITEM_INPUT, MENU_ITEM_LISTBOX, MENU_ITEM_SLIDER, MENU_ITEM_TEXT, MENU_ITEM_UNDEF,
    MenuItem, MenuItemKind, SEL_ACTION_BACK, SEL_ACTION_CLOSE, SEL_ACTION_CON_COMMANDS,
    SEL_ACTION_EXEC_COMMANDS, SEL_ACTION_PLAY_SOUND, SEL_ACTION_START_ITEM, SEL_ACTION_START_MENU,
    SEL_ACTION_UNDEF,
};
pub use npc::Npc;
pub use registry::{ClassKind, TypedInstance, TypedRegistry};
pub use symbol::{SymbolDecl, SymbolKind};
