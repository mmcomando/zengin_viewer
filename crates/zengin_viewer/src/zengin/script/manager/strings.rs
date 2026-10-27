use std::collections::HashMap;

use daedalus_wasm::symbol_store::{SymbolMeta, SymbolStore};

use super::super::bindings::exports::mmcomando::zengine_scripts::manage::SymbolInfo;
use super::super::state::{SymbolTable, WellKnownSymbols};
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
    /// typed symbol ids, and builds the host symbol store from the guest
    /// metadata and initial values. Must be called before script functions are
    /// executed, host calls made during initialization already verify ids and
    /// `get-symbol-values` only reports the initial values before any script
    /// runs.
    pub fn load_symbol_table(&mut self) {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        let symbols_count = guest.call_get_all_symbols_num(&mut self.store).unwrap();
        let word_count =
            usize::try_from(guest.call_get_symbols_memory_size(&mut self.store).unwrap() / 4)
                .unwrap_or_default();
        let mut symbol_store = SymbolStore::new(word_count);
        let mut kinds = HashMap::with_capacity(symbols_count.max(0) as usize);
        for index in 0..symbols_count {
            let info = guest.call_get_symbol_info(&mut self.store, index).unwrap();
            kinds.insert(index, info.kind);
            symbol_store.push_meta(SymbolMeta {
                kind: convert_symbol_kind(info.kind),
                is_const: info.is_const,
                is_class_member: info.is_class_member,
                address_or_offset: info.address_or_offset,
                has_function: info.function.is_some(),
            });
        }

        // Initial values of the symbols which have their own value slot. They
        // are only meaningful before instance initialization runs.
        for index in 0..symbols_count {
            let Some(meta) = symbol_store.meta(index as u32).copied() else {
                continue;
            };
            if meta.is_const || meta.is_class_member {
                continue;
            }
            let values = guest
                .call_get_symbol_values(&mut self.store, index)
                .unwrap();
            symbol_store.set_defaults(index as u32, &values);
        }

        // Load all string pair contents once at startup (symbol names for ids
        // below `symbols_count`, static string literals above), so import
        // handlers and host code resolve strings by table lookup instead of
        // calling back into the guest while a script function is running
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

        let well_known_symbols = WellKnownSymbols {
            self_symbol: guest.call_get_symbol_id(&mut self.store, "self").unwrap(),
            other_symbol: guest.call_get_symbol_id(&mut self.store, "other").unwrap(),
            victim_symbol: guest.call_get_symbol_id(&mut self.store, "victim").unwrap(),
            item_symbol: guest.call_get_symbol_id(&mut self.store, "item").unwrap(),
            hero_symbol: guest.call_get_symbol_id(&mut self.store, "hero").unwrap(),
        };

        let data = self.store.data_mut();
        data.symbol_store = symbol_store;
        data.well_known_symbols = well_known_symbols;
        data.symbol_table = SymbolTable::new(kinds, symbols_count, strings);
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

        self.has_ancestor(&symbol_info, ancestor_id)
    }
}

/// Maps the guest `symbol-kind` enum to the converter's kind enum used by the
/// host symbol store.
fn convert_symbol_kind(
    kind: super::super::bindings::exports::mmcomando::zengine_scripts::manage::SymbolKind,
) -> daedalus_wasm::daedalus::SymbolKind {
    use super::super::bindings::exports::mmcomando::zengine_scripts::manage::SymbolKind as Input;
    use daedalus_wasm::daedalus::SymbolKind as Output;
    match kind {
        Input::Float => Output::Float,
        Input::Int => Output::Int,
        Input::Str => Output::String,
        Input::Class => Output::Class,
        Input::Function => Output::Func,
        Input::Prototype => Output::Prototype,
        Input::Instance => Output::Instance,
        Input::VariableArgument => Output::VariableArgument,
    }
}
