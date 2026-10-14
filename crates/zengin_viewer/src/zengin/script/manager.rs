use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use daedalus_wasm::wasm::convert_daedalus_script;
use wasmtime::component::{Component, HasData, Linker};
use wasmtime::{Engine, Store};

use super::bindings::MmWorld;
use super::bindings::exports::mmcomando::zengine_scripts::manage::{
    DialogInfo, SymbolInfo, SymbolKind,
};
use super::state::{DialogLine, DialogRoot, ImportCallState, LoadedState, SymbolTable};
use super::symbol_id::{FunctionId, InstanceId, SymbolId};

#[allow(dead_code)]
pub struct ScriptManager {
    pub store: Store<ImportCallState>,
    pub instance: Arc<MmWorld>,
    /// Cache of resolved `_loop` function symbol ids of routine state
    /// functions, `None` when the state has no loop function.
    routine_loop_functions: HashMap<FunctionId, Option<FunctionId>>,
}

pub(crate) struct ScriptImports;

impl HasData for ScriptImports {
    type Data<'a> = &'a mut ImportCallState;
}

#[allow(dead_code)]
impl ScriptManager {
    pub fn get_loaded_state(&self) -> &LoadedState {
        &self.store.data().loaded_state
    }

    pub fn clear_dialog_lines(&mut self) {
        self.store.data_mut().dialog_state.dialog_lines.clear();
    }

    pub fn dialog_lines(&self) -> &[DialogLine] {
        &self.store.data().dialog_state.dialog_lines
    }

    pub fn clear_sub_dialog_choices(&mut self) {
        self.store
            .data_mut()
            .dialog_state
            .sub_dialog_choices
            .clear();
    }

    pub fn sub_dialog_choices(&self) -> &[DialogRoot] {
        &self.store.data().dialog_state.sub_dialog_choices
    }

    pub fn close_dialog(&self) -> bool {
        self.store.data().dialog_state.close_dialog
    }

    /// Registers the info instance as known by the npc, used when a dialog
    /// root is actually played
    pub fn mark_info_known(
        &mut self,
        npc_instance_id: InstanceId,
        info_instance_id: Option<InstanceId>,
    ) {
        let Some(info_instance_id) = info_instance_id else {
            return;
        };
        self.store
            .data_mut()
            .known_infos
            .entry(npc_instance_id)
            .or_default()
            .insert(info_instance_id);
    }

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

    pub fn set_instance(&mut self, instance_index: InstanceId) {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_set_instance(&mut self.store, instance_index.value())
            .unwrap()
    }

    /// Binds the given instance as `self` for subsequent script calls
    pub fn set_self(&mut self, instance_index: InstanceId) {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_set_self(&mut self.store, instance_index.value())
            .unwrap()
    }

    /// Binds the given instance as `other` for subsequent script calls
    pub fn set_other(&mut self, instance_index: InstanceId) {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_set_other(&mut self.store, instance_index.value())
            .unwrap()
    }

    pub fn call_npc_routine(&mut self, instance_index: InstanceId) {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        let result = guest.call_call_npc_routine(&mut self.store, instance_index.value());
        match result {
            Ok(_) => {}
            Err(err) => {
                println!("call_npc_routine({instance_index}) error: {:?}", err);
            }
        }
    }

    pub fn call_get_name(&mut self, symbol_id: SymbolId) -> String {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_get_name(&mut self.store, symbol_id.value())
            .unwrap()
    }

    pub fn get_string_non_0(&mut self, string_id: SymbolId) -> Option<String> {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        let string = guest
            .call_get_name(&mut self.store, string_id.value())
            .ok()?;
        if string.is_empty() {
            // TODO Empty strings should be a proper value.
            // Currently a lot of calls returns empty string which is probably a bug somewhere in scripts implementation.
            // To prevent further "head model no found errors" return None here.
            return None;
        }
        return Some(string);
    }

    pub fn get_symbol_id(&mut self, symbol_name: &str) -> SymbolId {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        let symbol_id = guest
            .call_get_symbol_id(&mut self.store, symbol_name)
            .unwrap()
            .expect("Symbol should exist");
        SymbolId::new(symbol_id).expect("Symbol id out of range")
    }

    /// Resolves a function symbol by name, verifying that it is a function.
    /// Panics when the symbol does not exist or is not a function.
    pub fn get_function_id(&mut self, symbol_name: &str) -> FunctionId {
        let symbol_id = self.get_symbol_id(symbol_name);
        self.symbol_table()
            .function(symbol_id.value())
            .expect("Symbol is not a function")
    }

    pub fn symbol_table(&self) -> &SymbolTable {
        &self.store.data().symbol_table
    }

    /// Verifies the raw id refers to an instance symbol
    pub fn instance_id(&self, raw: i32) -> Option<InstanceId> {
        self.symbol_table().instance(raw)
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
        self.store.data_mut().symbol_table = SymbolTable::new(kinds, symbols_count);
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

    fn try_call_function(&mut self, symbol_id: FunctionId) -> wasmtime::Result<()> {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest.call_call_function(&mut self.store, symbol_id.value())
    }

    /// Calls a function returning a value (`ZS_*_loop` functions do). The guest
    /// function table is typed, so such functions must use the bool call path
    /// or the call would trap on the signature mismatch.
    fn try_call_function_bool(&mut self, symbol_id: FunctionId) -> wasmtime::Result<bool> {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest.call_call_function_bool(&mut self.store, symbol_id.value())
    }

    /// Executes the routine state function (`ZS_*`) for the npc instance with
    /// the instance set as `self`, followed by a single invocation of the
    /// state's `_loop` function when one exists, mirroring how the engine
    /// starts a routine state. Returns the animation names the script requested
    /// via `AI_PlayAni(BS)` in request order.
    pub fn call_routine_state_function(
        &mut self,
        npc_instance_id: InstanceId,
        state_fn: FunctionId,
    ) -> Vec<String> {
        self.store
            .data_mut()
            .loaded_state
            .routine_played_anis
            .remove(&npc_instance_id);
        self.set_self(npc_instance_id);
        self.set_instance(npc_instance_id);
        if let Err(err) = self.try_call_function(state_fn) {
            println!(
                "Routine state function instance({npc_instance_id}) state_fn({}) error: {err:?}",
                state_fn.value()
            );
            return Vec::new();
        }

        if !self.routine_loop_functions.contains_key(&state_fn) {
            let loop_name = format!("{}_loop", self.call_get_name(state_fn.symbol()));
            let loop_fn = self
                .try_get_symbol_id(&loop_name)
                .and_then(|symbol| self.store.data().symbol_table.function(symbol.value()));
            self.routine_loop_functions.insert(state_fn, loop_fn);
        }
        let loop_fn = self.routine_loop_functions[&state_fn];
        if let Some(loop_fn) = loop_fn
            && let Err(err) = self.try_call_function_bool(loop_fn)
        {
            println!(
                "Routine state loop function instance({npc_instance_id}) loop_fn({}) error: {err:?}",
                loop_fn.value()
            );
        }

        self.store
            .data_mut()
            .loaded_state
            .routine_played_anis
            .remove(&npc_instance_id)
            .unwrap_or_default()
    }

    pub fn call_function(&mut self, symbol_id: FunctionId) {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_call_function(&mut self.store, symbol_id.value())
            .unwrap()
    }

    pub fn call_function_bool(&mut self, symbol_id: FunctionId) -> bool {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_call_function_bool(&mut self.store, symbol_id.value())
            .unwrap()
    }

    /// Evaluates the condition function of every root with the npc instance
    /// set, dropping roots whose condition returns false
    pub fn filter_dialog_roots(
        &mut self,
        npc_player: InstanceId,
        npc_talk_to: InstanceId,
        roots: &mut Vec<DialogRoot>,
    ) {
        roots.retain(|root| {
            let Some(condition_fn) = root.condition_function_id else {
                return true;
            };
            if !root.permanent
                && let Some(info_instance_id) = root.info_instance_id
                && let Some(infos) = self.store.data().known_infos.get(&npc_player)
                && infos.contains(&info_instance_id)
            {
                return false;
            }

            self.set_instance(npc_talk_to);
            self.set_self(npc_talk_to);
            self.set_other(npc_player);
            self.call_function_bool(condition_fn)
        });
    }

    pub fn initialize_instances(&mut self) {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_initialize_all_instances(&mut self.store)
            .unwrap()
    }

    pub fn get_symbol_info(&mut self, symbol_id: SymbolId) -> SymbolInfo {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_get_symbol_info(&mut self.store, symbol_id.value())
            .unwrap()
    }

    pub fn get_dialog_info(&mut self, symbol_id: SymbolId) -> DialogInfo {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_get_dialog_info(&mut self.store, symbol_id.value())
            .unwrap()
    }

    pub fn get_item_visual(&mut self, symbol_id: SymbolId) -> Option<String> {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_get_item_visual(&mut self.store, symbol_id.value())
            .unwrap()
    }
    pub fn get_item_visual_change(&mut self, symbol_id: SymbolId) -> Option<String> {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_get_item_visual_change(&mut self.store, symbol_id.value())
            .unwrap()
    }

    pub fn call_npc_routines(&mut self) {
        let npc_indices: Vec<_> = self.store.data().raw_instances.keys().cloned().collect();
        for index in npc_indices {
            self.set_self(index);
            self.set_instance(index);
            self.call_npc_routine(index);
        }
    }

    pub fn get_all_symbols_num(&mut self) -> i32 {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest.call_get_all_symbols_num(&mut self.store).unwrap()
    }

    fn has_ancestor(&mut self, symbol_info: &SymbolInfo, ancestor_id: SymbolId) -> bool {
        let Some(parent) = symbol_info.parent else {
            return false;
        };
        if parent == ancestor_id.value() {
            return true;
        }
        let symbol_info = self.get_symbol_info(SymbolId::new(parent).unwrap());

        return self.has_ancestor(&symbol_info, ancestor_id);
    }

    pub fn load_item_instances(&mut self) {
        let symbols_num = self.get_all_symbols_num();

        let item_class_id = self.get_symbol_id("c_item");

        for index in 0..symbols_num {
            let Some(index) = SymbolId::new(index) else {
                continue;
            };
            let symbol_info = self.get_symbol_info(index);
            if symbol_info.kind != SymbolKind::Instance {
                continue;
            }
            // Kind was just verified, the checked conversion cannot fail
            let index = InstanceId::from_symbol(index);

            if !self.has_ancestor(&symbol_info, item_class_id) {
                continue;
            }
            if symbol_info.function.is_none() {
                // Instance doesn't have init code, so it won't have meaningful data
                continue;
            }
            let symbol_name = self.call_get_name(index.symbol());

            let Some(visual) = self.get_item_visual(index.symbol()) else {
                println!("No visual for item({})", &symbol_name);
                continue;
            };

            let visual_change = self.get_item_visual_change(index.symbol());
            let entry = self
                .store
                .data_mut()
                .loaded_state
                .item_instances
                .entry(symbol_name)
                .or_default();
            entry.visual = visual;
            entry.visual_change = visual_change;
        }
    }

    pub fn load_dialogs_information(&mut self) {
        let symbols_num = self.get_all_symbols_num();

        let info_class_id = self.get_symbol_id("c_info");

        for index in 0..symbols_num {
            let Some(index) = SymbolId::new(index) else {
                continue;
            };
            let symbol_info = self.get_symbol_info(index);
            if symbol_info.kind != SymbolKind::Instance {
                continue;
            }
            // Kind was just verified, the checked conversion cannot fail
            let index = InstanceId::from_symbol(index);
            if !self.has_ancestor(&symbol_info, info_class_id) {
                continue;
            }

            let symbol_name = self.call_get_name(index.symbol());
            if symbol_name.starts_with("dia_lares") {
                continue;
            }
            let dialog_info = self.get_dialog_info(index.symbol());
            let Some(sequence_function_id) = self.symbol_table().function(dialog_info.information)
            else {
                continue;
            };
            let description = self
                .symbol_table()
                .string(dialog_info.description)
                .and_then(|id| self.get_string_non_0(id.symbol()))
                .unwrap_or("".to_string());

            let Some(npc_id) = self.symbol_table().instance(dialog_info.npc) else {
                continue;
            };
            let condition_function_id = self.symbol_table().function(dialog_info.condition);
            let entry = self
                .store
                .data_mut()
                .loaded_state
                .dialog_roots
                .entry(npc_id)
                .or_default();
            entry.push(DialogRoot {
                info_instance_id: Some(index),
                sequence_function_id,
                condition_function_id,
                permanent: dialog_info.permanent,
                description,
                important: dialog_info.important,
                order: dialog_info.order,
            });

            entry.sort_by_key(|el| el.order);
        }
    }

    pub fn call_dialog_sequence_function(&mut self, sequence_function_id: FunctionId) {
        self.store.data_mut().dialog_state.close_dialog = false;
        // TODO some dialog options are skipped as they cause problems
        if [15670, 15678, 20246].contains(&sequence_function_id.value()) {
            // Function has return value but should not have
            return;
        }
        if [18914, 21446, 21880, 23103].contains(&sequence_function_id.value()) {
            // Real problem with script
            return;
        }
        self.call_function(sequence_function_id);
    }

    pub fn load_npc_instances(&mut self) {
        let instances = self.store.data().raw_instances.clone();
        for (index, spawn_npc) in &instances {
            let Some(body_model) = spawn_npc
                .body_model
                .map(|id| id.symbol())
                .and_then(|id| self.get_string_non_0(id))
            else {
                println!(
                    "Not found body model string for npc({index}), body_model({})",
                    spawn_npc
                        .body_model
                        .map(|id| id.value())
                        .unwrap_or_default()
                );
                continue;
            };
            let armor_model = spawn_npc
                .armor_model
                .and_then(|id| self.get_string_non_0(id));
            let hierarchy = spawn_npc
                .hierarchy
                .map(|id| id.symbol())
                .and_then(|id| self.get_string_non_0(id));
            let head_model = spawn_npc
                .head_model
                .map(|id| id.symbol())
                .and_then(|id| self.get_string_non_0(id));

            // Sometimes there is model "Hum_Head_Babe." (dot at the end), engine or scripts bug?
            let head_model = head_model.map(|el| {
                if !el.contains('.') {
                    return el;
                }
                el.replace('.', "")
            });

            let face_texture = if spawn_npc.face_texture != 0 {
                Some(format!("HUM_HEAD_V{}_C0.TGA", spawn_npc.face_texture))
            } else {
                None
            };

            let body_texture = if spawn_npc.body_texture != 0 {
                Some(format!("HUM_BODY_NAKED_V{}_C0.TGA", spawn_npc.body_texture))
            } else {
                None
            };

            let entry = self
                .store
                .data_mut()
                .loaded_state
                .instance_data
                .entry(*index)
                .or_default();
            entry.body_model = body_model;
            entry.body_texture = body_texture;
            entry.head_model = head_model;
            entry.face_texture = face_texture;
            entry.armor_model = armor_model;
            entry.hierarchy = hierarchy;
        }
    }
}
