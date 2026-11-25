// Method groups live in sibling files, one `impl` block per domain.
#![allow(clippy::multiple_inherent_impl)]

mod body_state;
mod changes;
mod combat;
mod death;
mod dialog;
mod freepoints;
mod inventory;
mod locks;
mod menu;
mod mobs;
mod npc;
mod perception;
mod save;
mod strings;
mod world;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use daedalus_wasm::wasm::convert_daedalus_script;
use wasmtime::component::{Component, HasData, Linker};
use wasmtime::{Engine, Store};

use crate::save::format::fnv1a_64;

use super::ScriptCallStats;
use super::bindings::MmWorld;
use super::state::{ImportCallState, RoutineFunctions, ScriptMessages};
use super::symbol_id::FunctionId;
pub(crate) use combat::AttackKind;
pub(crate) use locks::TALENT_PICKLOCK;

pub struct ScriptManager {
    pub store: Store<ImportCallState>,
    pub instance: Arc<MmWorld>,
    /// Cache of resolved `_loop`/`_end` hooks of routine state functions.
    pub(super) routine_functions: HashMap<FunctionId, RoutineFunctions>,
    /// `startup_global` ran, it is a one-time script entry point.
    pub(super) global_startup_done: bool,
    /// Stable fingerprint of the raw `GOTHIC.DAT` bytes, used to reject saves
    /// made against a different script build. `0` when the source is unknown.
    script_fingerprint: u64,
    call_stats: ScriptCallStats,
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

/// Path of a generated script wasm cache, resolved against the workspace
/// `target` directory so it is independent of the process working directory.
pub(crate) fn wasm_cache_path(name: &str) -> PathBuf {
    let target = std::env::var_os("CARGO_TARGET_DIR").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target"),
        PathBuf::from,
    );
    target.join("script_cache").join(name)
}

impl ScriptManager {
    pub fn from_file_cached(
        script_path: &Path,
        script_wasm_cache_path: &Path,
        messages: ScriptMessages,
    ) -> Self {
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
        if let Some(parent) = script_wasm_cache_path.parent() {
            std::fs::create_dir_all(parent).unwrap_or_else(|error| {
                panic!(
                    "failed to create script cache directory {}: {error}",
                    parent.display()
                )
            });
        }
        std::fs::write(script_wasm_cache_path, &wasm_bytes).unwrap();
        let mut manager = Self::new(&wasm_bytes, messages);
        manager.script_fingerprint = fingerprint;
        manager
    }

    pub fn new(wasm_bytes: &[u8], messages: ScriptMessages) -> Self {
        let engine = Engine::default();
        let component = Component::from_binary(&engine, wasm_bytes).unwrap();
        let mut linker = Linker::new(&engine);
        MmWorld::add_to_linker::<ImportCallState, ScriptImports>(
            &mut linker,
            |state: &mut ImportCallState| -> &mut ImportCallState { state },
        )
        .unwrap();

        let my_state = ImportCallState {
            messages,
            ..ImportCallState::default()
        };
        let mut store = Store::new(&engine, my_state);
        let instance = MmWorld::instantiate(&mut store, &component, &linker).unwrap();
        let instance_arc = Arc::new(instance);

        Self {
            store,
            instance: instance_arc,
            routine_functions: HashMap::new(),
            global_startup_done: false,
            script_fingerprint: 0,
            call_stats: ScriptCallStats,
        }
    }

    /// Fingerprint of the loaded `GOTHIC.DAT` used by the save compatibility
    /// check. `0` when the vm was built from bytes without a known source.
    pub fn script_fingerprint(&self) -> u64 {
        self.script_fingerprint
    }

    /// Calls a function ignoring its result. The declared return type cannot be
    /// trusted to match how the engine calls a function: dialog `_info`
    /// functions are `func int` while the dialog code expects void, and some
    /// routine hooks declared `func void` (for example `zs_runtowp_end`) are
    /// compiled with the returns flag set. The guest picks the indirect call
    /// type from the symbol's flag instead.
    pub(super) fn try_call_function(&mut self, symbol_id: FunctionId) -> wasmtime::Result<()> {
        self.call_stats.record(symbol_id);
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
        self.call_stats.record(symbol_id);
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest.call_call_function_bool(&mut self.store, symbol_id.value())
    }

    pub fn call_function(&mut self, symbol_id: FunctionId) {
        self.call_stats.record(symbol_id);
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_call_function_discard(&mut self.store, symbol_id.value())
            .unwrap();
    }

    pub fn call_function_bool(&mut self, symbol_id: FunctionId) -> bool {
        self.call_stats.record(symbol_id);
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_call_function_bool(&mut self.store, symbol_id.value())
            .unwrap()
    }

    /// Logs the host-initiated script function call counts accumulated since the
    /// previous flush, see [`ScriptCallStats`].
    #[cfg(feature = "debug_perf")]
    pub(crate) fn flush_call_stats(&mut self) {
        self.call_stats.flush(&self.store.data().symbol_table);
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
                    .call_call_function_discard(&mut self.store, index as i32)
                    .unwrap();
            }
        }
    }
}
