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
mod damage;
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
pub use damage::{
    DAM_BARRIER, DAM_BLUNT, DAM_EDGE, DAM_FALL, DAM_FIRE, DAM_FLY, DAM_INDEX_BARRIER,
    DAM_INDEX_BLUNT, DAM_INDEX_EDGE, DAM_INDEX_FALL, DAM_INDEX_FIRE, DAM_INDEX_FLY,
    DAM_INDEX_MAGIC, DAM_INDEX_MAX, DAM_INDEX_POINT, DAM_INVALID, DAM_MAGIC, DAM_POINT,
    DAMAGE_TYPES, PROT_BARRIER, PROT_BLUNT, PROT_EDGE, PROT_FALL, PROT_FIRE, PROT_FLY, PROT_MAGIC,
    PROT_POINT,
};
pub use generic::GenericVariable;
pub use info::Info;
pub use item::{
    ITEM_2HD_AXE, ITEM_2HD_SWD, ITEM_AXE, ITEM_BOW, ITEM_CROSSBOW, ITEM_DAG, ITEM_SHIELD, ITEM_SWD,
    Item, ON_STATE_MAX,
};
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
pub use npc::{
    AIVAR_MAX, ATR_DEXTERITY, ATR_HITPOINTS, ATR_HITPOINTS_MAX, ATR_INDEX_MAX, ATR_MANA,
    ATR_MANA_MAX, ATR_REGENERATE_HP, ATR_REGENERATE_MANA, ATR_STRENGTH, HITCHANCE_MAX, MISSION_MAX,
    Npc, TALENT_1H, TALENT_2H, TALENT_UNKNOWN,
};
pub use registry::{ClassKind, TypedInstance, TypedRegistry};
pub use symbol::{SymbolDecl, SymbolKind};
