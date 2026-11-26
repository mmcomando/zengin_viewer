pub mod bindings;
pub mod bridge;
pub mod component;
pub mod host;
pub mod inventory;
pub mod journal;
pub mod manager;
#[allow(
    dead_code,
    reason = "menu fields are filled in as the menu screens are wired"
)]
pub mod menu;
pub use daedalus_typed::{
    MENU_FLAG_ALIGN_CENTER, MENU_FLAG_DONT_SCALE_DIM, MENU_FLAG_SHOW_INFO, MENU_ITEM_FLAG_DISABLED,
    MENU_ITEM_FLAG_TXT_CENTER, MENU_SCRIPT_DIV, SEL_ACTION_BACK, SEL_ACTION_CLOSE,
    SEL_ACTION_START_MENU,
};
pub use menu::{MenuDefinition, MenuItemDefinition, MenuScriptResource, ensure_menu_script};
pub mod state;
pub mod stats;
pub mod string_value;
pub mod symbol_id;

pub use stats::ScriptCallStats;

#[cfg(test)]
mod debug_zengin_scripts;
