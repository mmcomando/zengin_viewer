pub(crate) mod bindings;
pub(crate) mod bridge;
pub(crate) mod component;
pub(crate) mod host;
pub(crate) mod inventory;
pub(crate) mod journal;
pub(crate) mod manager;
#[allow(
    dead_code,
    reason = "menu fields are filled in as the menu screens are wired"
)]
pub(crate) mod menu;
pub use daedalus_typed::{
    MENU_FLAG_ALIGN_CENTER, MENU_FLAG_DONT_SCALE_DIM, MENU_FLAG_SHOW_INFO, MENU_ITEM_FLAG_DISABLED,
    MENU_ITEM_FLAG_TXT_CENTER, MENU_SCRIPT_DIV, SEL_ACTION_BACK, SEL_ACTION_CLOSE,
    SEL_ACTION_START_MENU,
};
pub use menu::{MenuDefinition, MenuItemDefinition, MenuScriptResource, ensure_menu_script};
pub(crate) mod state;
pub(crate) mod stats;
pub(crate) mod string_value;
pub(crate) mod symbol_id;

pub use stats::ScriptCallStats;

#[cfg(test)]
pub(crate) mod debug_zengin_scripts;
