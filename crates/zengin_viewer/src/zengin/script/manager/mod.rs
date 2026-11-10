// Method groups live in sibling files, one `impl` block per domain.
#![allow(clippy::multiple_inherent_impl)]

mod changes;
mod dialog;
mod inventory;
mod menu;
mod npc;
mod save;
mod strings;
mod world;

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use daedalus_wasm::wasm::convert_daedalus_script;
use wasmtime::component::{Component, HasData, Linker};
use wasmtime::{Engine, Store};

use crate::save::format::fnv1a_64;

use super::bindings::MmWorld;
use super::state::ImportCallState;
use super::symbol_id::FunctionId;

pub struct ScriptManager {
    pub store: Store<ImportCallState>,
    pub instance: Arc<MmWorld>,
    /// Cache of resolved `_loop` function symbol ids of routine state
    /// functions, `None` when the state has no loop function.
    pub(super) routine_loop_functions: HashMap<FunctionId, Option<FunctionId>>,
    /// `startup_global` ran, it is a one-time script entry point.
    pub(super) global_startup_done: bool,
    /// Stable fingerprint of the raw `GOTHIC.DAT` bytes, used to reject saves
    /// made against a different script build. `0` when the source is unknown.
    script_fingerprint: u64,
}

/// Display stats of an item instance for the inventory info panel, string
/// members resolved from the string table.
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
        let fingerprint = std::fs::read(script_path).map_or_else(
            |error| {
                bevy::log::warn!(
                    "Failed to read {} for the script fingerprint: {error}",
                    script_path.display()
                );
                0
            },
            |bytes| fnv1a_64(&bytes),
        );
        let wasm_bytes = convert_daedalus_script(script_path);
        std::fs::write(script_wasm_cache_path, &wasm_bytes).unwrap();
        let mut manager = Self::new(&wasm_bytes);
        manager.script_fingerprint = fingerprint;
        manager
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
        let instance_arc = Arc::new(instance);

        Self {
            store,
            instance: instance_arc,
            routine_loop_functions: HashMap::new(),
            global_startup_done: false,
            script_fingerprint: 0,
        }
    }

    /// Fingerprint of the loaded `GOTHIC.DAT` used by the save compatibility
    /// check. `0` when the vm was built from bytes without a known source.
    pub fn script_fingerprint(&self) -> u64 {
        self.script_fingerprint
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

    /// Runs every daedalus instance initializer in symbol order, binding the
    /// instance as `self` and as the current instance context first. This used
    /// to be a guest builtin, it lives here because the symbol values are host
    /// owned now.
    pub fn initialize_instances(&mut self) {
        let self_symbol = self.store.data().well_known_symbols.self_symbol;
        let instances = {
            let typed = &self.store.data().typed;
            typed
                .instance_symbols()
                .map(|index| (index, typed.has_function(index)))
                .collect::<Vec<_>>()
        };
        for (index, has_function) in instances {
            if let Some(self_symbol) = self_symbol {
                self.store
                    .data_mut()
                    .typed
                    .write_symbol(self_symbol as u32, 0, index as i32);
            }
            let guest = self.instance.mmcomando_zengine_scripts_manage();
            guest
                .call_set_instance(&mut self.store, index as i32)
                .unwrap();
            guest.call_set_self(&mut self.store, index as i32).unwrap();
            if has_function {
                guest
                    .call_call_function(&mut self.store, index as i32)
                    .unwrap();
            }
        }
    }
}
