// Method groups live in sibling files, one `impl` block per domain.
#![allow(clippy::multiple_inherent_impl)]

mod changes;
mod dialog;
mod inventory;
mod npc;
mod strings;

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use daedalus_wasm::wasm::convert_daedalus_script;
use wasmtime::component::{Component, HasData, Linker};
use wasmtime::{Engine, Store};

use super::bindings::MmWorld;
use super::state::ImportCallState;
use super::symbol_id::FunctionId;

pub struct ScriptManager {
    pub store: Store<ImportCallState>,
    pub instance: Arc<MmWorld>,
    /// Cache of resolved `_loop` function symbol ids of routine state
    /// functions, `None` when the state has no loop function.
    pub(super) routine_loop_functions: HashMap<FunctionId, Option<FunctionId>>,
}

/// Display stats of an item instance for the inventory info panel, string
/// members resolved from the string table. Weight is currently not shown (the
/// G2 script info lines cover it).
#[derive(Debug, Default)]
pub struct ItemDisplayData {
    pub damage_total: i32,
    /// Base value (`C_Item.value`).
    pub value: i32,
    /// `C_Item.description` flavor text, `None` when unset.
    pub description: Option<String>,
    /// Equip requirements as (attribute display name, value) pairs.
    pub requirements: Vec<(&'static str, i32)>,
    /// Generic info-box lines as (text, value) pairs, unset lines skipped.
    pub info_lines: Vec<(String, i32)>,
}

pub(crate) struct ScriptImports;

impl HasData for ScriptImports {
    type Data<'a> = &'a mut ImportCallState;
}

/// Display names of the daedalus attribute indices items can require.
pub(super) fn attribute_display_name(index: i32) -> Option<&'static str> {
    match index {
        // G2 script constants: ATR_STRENGTH = 4, ATR_DEXTERITY = 5
        4 => Some("Strength"),
        5 => Some("Dexterity"),
        _ => None,
    }
}

impl ScriptManager {
    pub fn from_file_cached(script_path: &Path, script_wasm_cache_path: &Path) -> Self {
        // if script_wasm_cache_path.is_file() {
        //     let wasm_bytes = std::fs::read(script_wasm_cache_path).unwrap();
        //     return Self::new(&wasm_bytes);
        // }
        let wasm_bytes = convert_daedalus_script(script_path);
        std::fs::write(script_wasm_cache_path, &wasm_bytes).unwrap();
        return Self::new(&wasm_bytes);
    }

    pub fn new(wasm_bytes: &[u8]) -> Self {
        let engine = Engine::default();
        let component = Component::from_binary(&engine, wasm_bytes).unwrap();
        let mut linker = Linker::new(&engine);
        MmWorld::add_to_linker::<ImportCallState, ScriptImports>(
            &mut linker,
            |state: &mut ImportCallState| -> &mut ImportCallState { state },
        )
        .unwrap();

        let my_state = ImportCallState::default();
        let mut store = Store::new(&engine, my_state);
        let instance = MmWorld::instantiate(&mut store, &component, &linker).unwrap();
        let instance2 = MmWorld::instantiate(&mut store, &component, &linker).unwrap();
        let instance_arc = Arc::new(instance);
        store.data_mut().world = Some(Arc::new(instance2));

        Self {
            store,
            instance: instance_arc,
            routine_loop_functions: HashMap::new(),
        }
    }

    pub(super) fn try_call_function(&mut self, symbol_id: FunctionId) -> wasmtime::Result<()> {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest.call_call_function(&mut self.store, symbol_id.value())
    }

    /// Calls a function ignoring its result. Dialog functions can be declared
    /// `func int` while the dialog code expects void, so the guest picks the
    /// indirect call type based on the symbol's return flag.
    pub(super) fn try_call_function_discard(
        &mut self,
        symbol_id: FunctionId,
    ) -> wasmtime::Result<()> {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest.call_call_function_discard(&mut self.store, symbol_id.value())
    }

    /// Calls a function returning a value (`ZS_*_loop` functions do). The guest
    /// function table is typed, so such functions must use the bool call path
    /// or the call would trap on the signature mismatch.
    pub(super) fn try_call_function_bool(
        &mut self,
        symbol_id: FunctionId,
    ) -> wasmtime::Result<bool> {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest.call_call_function_bool(&mut self.store, symbol_id.value())
    }

    pub fn call_function(&mut self, symbol_id: FunctionId) {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_call_function(&mut self.store, symbol_id.value())
            .unwrap();
    }

    pub fn call_function_bool(&mut self, symbol_id: FunctionId) -> bool {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_call_function_bool(&mut self.store, symbol_id.value())
            .unwrap()
    }

    pub fn initialize_instances(&mut self) {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_initialize_all_instances(&mut self.store)
            .unwrap();
    }
}
