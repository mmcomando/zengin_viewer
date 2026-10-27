use daedalus_typed::{Menu, MenuItem};

use super::super::symbol_id::SymbolId;

impl super::ScriptManager {
    /// Projects the `C_MENU` fields of a menu section instance.
    pub fn get_menu_info(&mut self, symbol: SymbolId) -> Option<Menu> {
        self.store.data().typed.menu(symbol.value() as u32)
    }

    /// Projects the `C_MENU_ITEM` fields of a menu item instance.
    pub fn get_menu_item_info(&mut self, symbol: SymbolId) -> Option<MenuItem> {
        self.store.data().typed.menu_item(symbol.value() as u32)
    }
}
