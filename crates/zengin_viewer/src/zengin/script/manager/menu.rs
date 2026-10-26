use super::super::bindings::exports::mmcomando::zengine_scripts::manage::{MenuInfo, MenuItemInfo};
use super::super::symbol_id::SymbolId;

#[allow(dead_code, reason = "used by the menu projection module")]
impl super::ScriptManager {
    /// Projects the `C_MENU` fields of a menu section instance.
    pub fn get_menu_info(&mut self, symbol: SymbolId) -> MenuInfo {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_get_menu_info(&mut self.store, symbol.value())
            .unwrap()
    }

    /// Projects the `C_MENU_ITEM` fields of a menu item instance.
    pub fn get_menu_item_info(&mut self, symbol: SymbolId) -> MenuItemInfo {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_get_menu_item_info(&mut self.store, symbol.value())
            .unwrap()
    }
}
