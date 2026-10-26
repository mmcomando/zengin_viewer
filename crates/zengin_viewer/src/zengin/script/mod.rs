pub mod bindings;
pub mod bridge;
pub mod component;
pub mod host;
pub mod inventory;
pub mod journal;
pub mod manager;
#[allow(
    dead_code,
    reason = "menu constants are filled in as the menu screens are wired"
)]
mod menu;
pub(crate) use menu::{
    MENU_FLAG_ALIGN_CENTER, MENU_FLAG_DONT_SCALE_DIM, MENU_FLAG_SHOW_INFO, MENU_ITEM_FLAG_DISABLED,
    MENU_ITEM_FLAG_TXT_CENTER, MENU_SCRIPT_DIV, MenuDefinition, MenuItemDefinition,
    MenuScriptResource, SEL_ACTION_CLOSE, SEL_ACTION_START_MENU,
};
pub mod state;
pub mod string_value;
pub mod symbol_id;

#[cfg(test)]
mod debug_zengin_scripts;

use bevy::prelude::*;

use crate::zengin::script::symbol_id::InstanceId;

/// Requests moving all items of one instance's inventory into another's. Written
/// by gameplay systems, applied by `handle_transfer_items` in the script bridge.
#[derive(Message)]
pub struct TransferItems {
    pub instance_from_id: InstanceId,
    pub instance_to_id: InstanceId,
}
