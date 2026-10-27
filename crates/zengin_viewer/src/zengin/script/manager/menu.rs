use daedalus_wasm::symbol_store::{self, MenuInfo, MenuItemInfo};

use super::super::symbol_id::SymbolId;

impl super::ScriptManager {
    /// Projects the `C_MENU` fields of a menu section instance.
    pub fn get_menu_info(&mut self, symbol: SymbolId) -> MenuInfo {
        symbol_store::menu_info(
            &self.store.data().symbol_store,
            u32::try_from(symbol.value()).unwrap_or_default(),
        )
    }

    /// Projects the `C_MENU_ITEM` fields of a menu item instance.
    pub fn get_menu_item_info(&mut self, symbol: SymbolId) -> MenuItemInfo {
        symbol_store::menu_item_info(
            &self.store.data().symbol_store,
            u32::try_from(symbol.value()).unwrap_or_default(),
        )
    }
}
