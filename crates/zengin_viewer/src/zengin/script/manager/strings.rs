use std::collections::HashMap;

use super::super::bindings::exports::mmcomando::zengine_scripts::manage::SymbolInfo;
use super::super::state::SymbolTable;
use super::super::string_value::{ShowString, StringCanon};
use super::super::symbol_id::{FunctionId, InstanceId, StringId, SymbolId};

impl super::ScriptManager {
    pub fn call_get_name(&mut self, symbol_id: SymbolId) -> String {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_get_name(&mut self.store, symbol_id.value())
            .unwrap()
    }

    /// Fetches the content of the string table entry `raw`, `None` only when
    /// the id is out of range. Empty strings are valid values, callers filter
    /// them out when emptiness means "none". Only works for static string
    /// literal ids, resolved by a table loaded at startup instead of calling
    /// into the guest.
    pub(super) fn get_string_non_0(&self, raw: i32) -> Option<String> {
        self.store.data().symbol_table.string_content(raw)
    }

    /// Fetches a static string literal with its original case, `None` when it
    /// is empty
    pub fn get_show_string(&mut self, id: StringId) -> Option<ShowString> {
        let string = self.get_string_non_0(id.value())?;
        Some(ShowString::new(string))
    }

    /// Fetches a static string literal as a canonical (lowercased) lookup key,
    /// for names matched case-insensitively (waypoints, spots), `None` when it
    /// is empty
    pub fn get_canon_string(&mut self, id: StringId) -> Option<StringCanon> {
        let string = self.get_string_non_0(id.value())?;
        Some(StringCanon::new(string))
    }

    /// Resolves the name of a raw symbol id, `None` when it is not a valid id.
    pub fn get_name_by_id(&mut self, raw: i32) -> Option<String> {
        let symbol = SymbolId::new(raw)?;
        Some(self.call_get_name(symbol))
    }

    /// Content of a daedalus string id, `None` when the id is unset. Empty
    /// strings are valid values.
    pub(crate) fn string_by_id(&self, raw: Option<i32>) -> Option<String> {
        self.get_string_non_0(raw?)
    }

    pub fn get_symbol_id(&mut self, symbol_name: &str) -> SymbolId {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        let symbol_id = guest
            .call_get_symbol_id(&mut self.store, symbol_name)
            .unwrap()
            .expect("Symbol should exist");
        SymbolId::new(symbol_id).expect("Symbol id out of range")
    }

    /// Resolves a callable symbol by name, `None` when the symbol does not
    /// exist or is not a function.
    pub fn try_get_function_id(&mut self, symbol_name: &str) -> Option<FunctionId> {
        let symbol = self.try_get_symbol_id(symbol_name)?;
        self.symbol_table().function(symbol.value())
    }

    /// Whether the instance symbol descends from the named class.
    pub fn is_instance_of_class(&mut self, instance: InstanceId, class_name: &str) -> bool {
        let Some(class) = self.try_get_symbol_id(class_name) else {
            return false;
        };
        let info = self.get_symbol_info(instance.symbol());
        self.has_ancestor(&info, class)
    }

    pub fn symbol_table(&self) -> &SymbolTable {
        &self.store.data().symbol_table
    }

    /// Resolves an instance symbol by name, `None` when the symbol does not
    /// exist or is not an instance.
    pub fn symbol_instance(&mut self, symbol_name: &str) -> Option<InstanceId> {
        let symbol = self.try_get_symbol_id(symbol_name)?;
        self.symbol_table().instance(symbol.value())
    }

    /// Loads the kind of every symbol into the symbol table used to verify
    /// typed symbol ids. Must be called before script functions are executed,
    /// host calls made during initialization already verify ids.
    pub fn load_symbol_table(&mut self) {
        let symbols_count = self.get_all_symbols_num();
        let mut kinds = HashMap::with_capacity(symbols_count.max(0) as usize);
        for index in 0..symbols_count {
            let Some(symbol_id) = SymbolId::new(index) else {
                continue;
            };
            kinds.insert(index, self.get_symbol_info(symbol_id).kind);
        }

        // Load all string pair contents once at startup (symbol names for ids
        // below `symbols_count`, static string literals above), so import
        // handlers and host code resolve strings by table lookup instead of
        // calling back into the guest while a script function is running
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        let strings_count = guest.call_get_all_strings_num(&mut self.store).unwrap();
        let mut strings = Vec::with_capacity(strings_count.max(0) as usize);
        for raw in 0..strings_count {
            let string = guest
                .call_get_name(&mut self.store, raw)
                .unwrap_or_else(|error| {
                    panic!("get_name({raw}) failed while loading the symbol table: {error}")
                });
            strings.push(string);
        }

        self.store.data_mut().symbol_table = SymbolTable::new(kinds, symbols_count, strings);
    }

    /// Resolves a symbol id by name, `None` when the symbol does not exist.
    /// The guest lookup does not trap for missing symbols, a trap would make
    /// the component instance unusable for all further calls.
    pub fn try_get_symbol_id(&mut self, symbol_name: &str) -> Option<SymbolId> {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_get_symbol_id(&mut self.store, symbol_name)
            .unwrap()
            .and_then(SymbolId::new)
    }

    pub fn get_symbol_info(&mut self, symbol_id: SymbolId) -> SymbolInfo {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_get_symbol_info(&mut self.store, symbol_id.value())
            .unwrap()
    }

    pub fn get_all_symbols_num(&mut self) -> i32 {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest.call_get_all_symbols_num(&mut self.store).unwrap()
    }

    pub(super) fn has_ancestor(&mut self, symbol_info: &SymbolInfo, ancestor_id: SymbolId) -> bool {
        let Some(parent) = symbol_info.parent else {
            return false;
        };
        if parent == ancestor_id.value() {
            return true;
        }
        let symbol_info = self.get_symbol_info(SymbolId::new(parent).unwrap());

        return self.has_ancestor(&symbol_info, ancestor_id);
    }
}
