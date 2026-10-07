use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use daedalus_wasm::wasm::convert_daedalus_script;
use wasmtime::{AsContextMut, Engine, Store};

use wasmtime::component::{Component, HasData, Linker, bindgen};

use crate::zengin::script::state::{
    DialogLine, DialogRoot, DialogState, LoadedState, RoutineEntry, SpawnItem, SpawnNpc,
};
use exports::mmcomando::zengine_scripts::manage::SymbolKind;

bindgen!({
    path:"wit",

    imports: {
        "mmcomando:zengine-scripts/scripts.wld-insertnpc": store,
        "mmcomando:zengine-scripts/scripts.mdl-setvisualbody": store,
        "mmcomando:zengine-scripts/scripts.mdl-setvisual": store,
        "mmcomando:zengine-scripts/scripts.ta-min": store,
        "mmcomando:zengine-scripts/scripts.wld-insertitem": store,
        "mmcomando:zengine-scripts/scripts.ai-output": store,
        "mmcomando:zengine-scripts/scripts.ai-stopprocessinfos": store,


    },

});

#[derive(Debug, Default, Clone)]
pub struct InstanceStateRaw {
    pub body_model: i32,
    pub body_texture: i32,
    pub head_model: i32,
    pub face_texture: i32,
    pub armor_model: i32,
    pub hierarchy: i32,
}

#[derive(Default)]
pub struct ImportCallState {
    pub world: Option<Arc<MmWorld>>,
    pub loaded_state: LoadedState,
    pub dialog_state: DialogState,
    pub raw_instances: HashMap<u32, InstanceStateRaw>,
}

#[allow(dead_code)]
pub struct WasmExecutor {
    pub store: Store<ImportCallState>,
    pub instance: Arc<MmWorld>,
}

struct MyLibrary;

impl HasData for MyLibrary {
    type Data<'a> = &'a mut ImportCallState;
}

#[allow(dead_code)]
impl WasmExecutor {
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
        MmWorld::add_to_linker::<ImportCallState, MyLibrary>(
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
        }
    }

    pub fn set_instance(&mut self, instance_index: u32) {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_set_instance(&mut self.store, instance_index as i32)
            .unwrap()
    }

    pub fn call_npc_routine(&mut self, instance_index: u32) {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        let result = guest.call_call_npc_routine(&mut self.store, instance_index as i32);
        match result {
            Ok(_) => {}
            Err(err) => {
                println!("call_npc_routine({instance_index}) error: {:?}", err);
            }
        }
    }

    pub fn call_get_name(&mut self, symbol_id: u32) -> String {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_get_name(&mut self.store, symbol_id as i32)
            .unwrap()
    }

    pub fn get_string_non_0(&mut self, string_id: i32) -> Option<String> {
        if string_id <= 0 {
            return None;
        }
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        let string = guest
            .call_get_name(&mut self.store, string_id as i32)
            .ok()?;
        if string.is_empty() {
            // TODO Empty strings should be a proper value.
            // Currently a lot of calls returns empty string which is probably a bug somewhere in scripts implementation.
            // To prevent further "head model no found errors" return None here.
            return None;
        }
        return Some(string);
    }

    pub fn get_symbol_id(&mut self, symbol_name: &str) -> i32 {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_get_symbol_id(&mut self.store, symbol_name)
            .unwrap()
    }

    pub fn call_function(&mut self, symbol_id: i32) {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_call_function(&mut self.store, symbol_id)
            .unwrap()
    }

    pub fn initialize_instances(&mut self) {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_initialize_all_instances(&mut self.store)
            .unwrap()
    }

    pub fn get_symbol_info(
        &mut self,
        symbol_id: i32,
    ) -> crate::zengin::script::main::exports::mmcomando::zengine_scripts::manage::SymbolInfo {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_get_symbol_info(&mut self.store, symbol_id)
            .unwrap()
    }

    pub fn get_dialog_info(
        &mut self,
        symbol_id: i32,
    ) -> crate::zengin::script::main::exports::mmcomando::zengine_scripts::manage::DialogInfo {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_get_dialog_info(&mut self.store, symbol_id)
            .unwrap()
    }

    pub fn get_item_visual(&mut self, symbol_id: i32) -> Option<String> {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_get_item_visual(&mut self.store, symbol_id)
            .unwrap()
    }
    pub fn get_item_visual_change(&mut self, symbol_id: i32) -> Option<String> {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_get_item_visual_change(&mut self.store, symbol_id)
            .unwrap()
    }

    pub fn call_npc_routines(&mut self) {
        let npc_indices: Vec<_> = self.store.data().raw_instances.keys().cloned().collect();
        for index in npc_indices {
            self.set_instance(index);
            self.call_npc_routine(index as u32);
        }
    }

    pub fn get_all_symbols_num(&mut self) -> i32 {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest.call_get_all_symbols_num(&mut self.store).unwrap()
    }

    fn has_ancestor(
        &mut self,
        symbol_info: &crate::zengin::script::main::exports::mmcomando::zengine_scripts::manage::SymbolInfo,
        ancestor_id: i32,
    ) -> bool {
        let Some(parent) = symbol_info.parent else {
            return false;
        };
        if parent == ancestor_id {
            return true;
        }
        let symbol_info = self.get_symbol_info(parent);

        return self.has_ancestor(&symbol_info, ancestor_id);
    }

    pub fn load_item_instances(&mut self) {
        let symbols_num = self.get_all_symbols_num();

        let item_class_id = self.get_symbol_id("c_item");

        for index in 0..symbols_num {
            let symbol_info = self.get_symbol_info(index);
            if symbol_info.kind != SymbolKind::Instance {
                continue;
            }

            if !self.has_ancestor(&symbol_info, item_class_id) {
                continue;
            }
            if symbol_info.function.is_none() {
                // Instance doesn't have init code, so it won't have meaningful data
                continue;
            }
            let symbol_name = self.call_get_name(index as u32);

            let Some(visual) = self.get_item_visual(index) else {
                println!("No visual for item({})", &symbol_name);
                continue;
            };

            let visual_change = self.get_item_visual_change(index);
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
            let symbol_info = self.get_symbol_info(index);
            if symbol_info.kind != SymbolKind::Instance {
                continue;
            }
            if !self.has_ancestor(&symbol_info, info_class_id) {
                continue;
            }

            let symbol_name = self.call_get_name(index as u32);
            if symbol_name.starts_with("dia_lares") {
                continue;
            }
            let dialog_info = self.get_dialog_info(index);
            let description = self
                .get_string_non_0(dialog_info.description)
                .unwrap_or("".to_string());

            let entry = self
                .store
                .data_mut()
                .loaded_state
                .dialog_roots
                .entry(dialog_info.npc as u32)
                .or_default();
            entry.push(DialogRoot {
                sequence_function_id: dialog_info.information,
                description,
                important: dialog_info.important,
                order: dialog_info.order,
            });

            entry.sort_by_key(|el| el.order);
        }
    }

    pub fn call_dialog_sequence_function(&mut self, sequence_function_id: i32) {
        // TODO some dialog options are skipped as they cause problems
        if [15670, 15678, 20246].contains(&sequence_function_id) {
            // Function has return value but should not have
            return;
        }
        if [18914, 21446, 21880, 23103].contains(&sequence_function_id) {
            // Real problem with script
            return;
        }
        self.call_function(sequence_function_id);
    }

    pub fn load_npc_instances(&mut self) {
        let instances = self.store.data().raw_instances.clone();
        for (index, spawn_npc) in &instances {
            let Some(body_model) = self.get_string_non_0(spawn_npc.body_model) else {
                println!(
                    "Not found body model string for npc({index}), body_model_index({})",
                    spawn_npc.body_model
                );
                continue;
            };
            let armor_model = self.get_string_non_0(spawn_npc.armor_model);
            let hierarchy = self.get_string_non_0(spawn_npc.hierarchy);
            let head_model = self.get_string_non_0(spawn_npc.head_model);

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

macro_rules! count_import {
    ($name:ident($($arg:ident),*)) => {
        fn $name(&mut self, $($arg: i32),*) {
        }
    };
    ($name:ident($($arg:ident),*) -> i32) => {
        fn $name(&mut self, $($arg: i32),*) -> i32 {
            0
        }
    };

}

impl MyLibrary {
    fn get_string_non_0(
        access: &mut wasmtime::component::Access<'_, ImportCallState, MyLibrary>,
        string_id: i32,
    ) -> Option<String> {
        if string_id == 0 {
            return None;
        }
        let world = access.get().world.clone().unwrap();
        let store_context = access.as_context_mut();
        let string = world
            .mmcomando_zengine_scripts_manage()
            .call_get_name(store_context, string_id)
            .unwrap();
        // .ok()?;
        Some(string)
    }

    fn call_function(
        access: &mut wasmtime::component::Access<'_, ImportCallState, MyLibrary>,
        symbol_id: i32,
    ) {
        let world = access.get().world.clone().unwrap();
        let store_context = access.as_context_mut();
        world
            .mmcomando_zengine_scripts_manage()
            .call_call_function(store_context, symbol_id)
            .unwrap();
    }

    fn set_instance(
        access: &mut wasmtime::component::Access<'_, ImportCallState, MyLibrary>,
        symbol_id: i32,
    ) {
        let world = access.get().world.clone().unwrap();
        let store_context = access.as_context_mut();
        world
            .mmcomando_zengine_scripts_manage()
            .call_set_instance(store_context, symbol_id)
            .unwrap();
    }
}

impl mmcomando::zengine_scripts::scripts::HostWithStore<ImportCallState> for MyLibrary {
    fn wld_insertnpc(
        mut access: wasmtime::component::Access<'_, ImportCallState, MyLibrary>,
        npc_index: i32,
        waypoint_string_id: i32,
    ) {
        Self::set_instance(&mut access, npc_index);
        Self::call_function(&mut access, npc_index);
        // let npc_name = Self::get_string_non_0(&mut access, npc_index).unwrap();
        let waypoint_name = Self::get_string_non_0(&mut access, waypoint_string_id)
            .unwrap()
            .to_lowercase();

        // println!(
        //     "NEW wld_insertnpc npc_index({npc_index}), npc_name({npc_name}), waypoint_string_id({waypoint_string_id}), waypoint_name({waypoint_name})"
        // );

        let npc = SpawnNpc {
            npc_index: npc_index as u32,
            way_point: waypoint_name,
        };
        let state = access.data_mut();
        state.loaded_state.spawn_npcs.push(npc);
    }

    fn wld_insertitem(
        mut access: wasmtime::component::Access<'_, ImportCallState, MyLibrary>,
        item_index: i32,
        waypoint_string_id: i32,
    ) {
        // println!("CCC");
        Self::set_instance(&mut access, item_index);
        Self::call_function(&mut access, item_index);
        let item_name = Self::get_string_non_0(&mut access, item_index).unwrap();
        let waypoint_name = Self::get_string_non_0(&mut access, waypoint_string_id)
            .unwrap()
            .to_ascii_lowercase();
        // println!(
        //     "NEW wld_insertitem item_index({item_index}), item_name({item_name}), waypoint_string_id({waypoint_string_id}), waypoint_name({waypoint_name})"
        // );

        let state = access.data_mut();
        state.loaded_state.spawn_items.push(SpawnItem {
            instance_name: item_name,
            way_point: waypoint_name,
        });
    }

    fn mdl_setvisualbody(
        mut access: wasmtime::component::Access<'_, ImportCallState, MyLibrary>,
        npc_index: i32,
        body_model: i32,
        body_texture_index: i32,
        _par_4: i32,
        head_model: i32,
        face_texture_index: i32,
        _par_1: i32,
        armor_model: i32,
    ) {
        // println!(
        //     "NEW mdl_setvisualbody npc_index({npc_index}), body_model_id({body_model}), body_texture_index({body_texture_index}), head_model({head_model}), face_texture_index({face_texture_index}), armor_index({armor_model})"
        // );

        let state = access.data_mut();
        let entry = state.raw_instances.entry(npc_index as u32).or_default();
        entry.body_model = body_model;
        entry.head_model = head_model;
        entry.armor_model = armor_model;
        entry.face_texture = face_texture_index;
        entry.body_texture = body_texture_index;
    }

    fn mdl_setvisual(
        mut access: wasmtime::component::Access<'_, ImportCallState, MyLibrary>,
        npc_index: i32,
        body_model_id: i32,
    ) {
        // println!("NEW mdl_setvisual npc_index({npc_index}), body_model_id({body_model_id})");
        let state = access.data_mut();
        let entry = state.raw_instances.entry(npc_index as u32).or_default();
        entry.hierarchy = body_model_id;
    }

    fn ta_min(
        mut access: wasmtime::component::Access<'_, ImportCallState, MyLibrary>,
        npc_index: i32,
        start_h: i32,
        _start_m: i32,
        stop_h: i32,
        _stop_m: i32,
        _func_index: i32,
        way_point: i32,
    ) {
        // println!(
        //     "NEW ta_min npc_index({npc_index}), start_h({start_h}), stop_h({stop_h}), way_point({way_point})"
        // );
        let way_point = Self::get_string_non_0(&mut access, way_point)
            .unwrap()
            .to_lowercase();

        let start_h = start_h as u32;
        let stop_h = stop_h as u32;
        let routine_entry = RoutineEntry {
            start_h,
            stop_h,
            way_point,
        };

        let state = access.data_mut();
        let entry = state
            .loaded_state
            .instance_data
            .entry(npc_index as u32)
            .or_default();
        entry.routine_enties.push(routine_entry);
    }

    fn ai_output(
        mut access: wasmtime::component::Access<'_, ImportCallState, MyLibrary>,
        npc_a: i32,
        npc_b: i32,
        line_id: i32,
    ) {
        let line_str = Self::get_string_non_0(&mut access, line_id)
            .unwrap()
            .to_lowercase();
        // println!(
        //     "ai_output npc_a({npc_a}), npc_b({npc_b}), line_id({line_id}), line_str({line_str})"
        // );
        let state = access.data_mut();
        state.dialog_state.dialog_lines.push(DialogLine {
            npc_a,
            npc_b,
            line_id,
            line_str,
        });
    }

    fn ai_stopprocessinfos(
        mut access: wasmtime::component::Access<'_, ImportCallState, MyLibrary>,
        _npc: i32,
    ) {
        let state = access.data_mut();
        // TODO send event?
        state.dialog_state.close_dialog = true;
    }
}

impl mmcomando::zengine_scripts::scripts::Host for ImportCallState {
    count_import!(inttostring(_0) -> i32);
    count_import!(floattostring(_0) -> i32);
    count_import!(floattoint(_0) -> i32);
    count_import!(inttofloat(_0) -> i32);
    count_import!(concatstrings(_0, _1) -> i32);
    count_import!(print(_0));
    count_import!(printdebug(_0));
    count_import!(printscreen(_0, _1, _2, _3, _4) -> i32);
    count_import!(ai_printscreen(_0, _1, _2, _3, _4) -> i32);
    count_import!(printdialog(_0, _1, _2, _3, _4, _5) -> i32);
    count_import!(printdebuginst(_0));
    count_import!(printdebuginstch(_0, _1));
    count_import!(printdebugch(_0, _1));
    count_import!(printmulti(_0, _1, _2, _3, _4));
    count_import!(exitgame());
    count_import!(exitsession());
    count_import!(playvideo(_0) -> i32);
    count_import!(playvideoex(_0, _1, _2) -> i32);
    count_import!(setpercentdone(_0));
    count_import!(introducechapter(_0, _1, _2, _3, _4));
    count_import!(npc_memoryentry(_0, _1, _2, _3, _4));
    count_import!(npc_memoryentryguild(_0, _1, _2, _3, _4));
    count_import!(npc_hasnews(_0, _1, _2, _3) -> i32);
    count_import!(npc_getnewswitness(_0, _1) -> i32);
    count_import!(npc_getnewsoffender(_0, _1) -> i32);
    count_import!(npc_getnewsvictim(_0, _1) -> i32);
    count_import!(npc_isnewsgossip(_0, _1) -> i32);
    count_import!(npc_deletenews(_0, _1) -> i32);
    count_import!(npc_getguildattitude(_0, _1) -> i32);
    count_import!(npc_getactivespell(_0) -> i32);
    count_import!(npc_getlasthitspellid(_0) -> i32);
    count_import!(npc_getlasthitspellcat(_0) -> i32);
    count_import!(npc_getactivespellcat(_0) -> i32);
    count_import!(npc_getactivespelllevel(_0) -> i32);
    count_import!(npc_getactivespellisscroll(_0) -> i32);
    count_import!(npc_setactivespellinfo(_0, _1) -> i32);
    count_import!(wld_detectitem(_0, _1) -> i32);
    count_import!(wld_detectplayer(_0) -> i32);
    count_import!(wld_detectnpc(_0, _1, _2, _3) -> i32);
    count_import!(wld_detectnpcex(_0, _1, _2, _3, _4) -> i32);
    count_import!(wld_detectnpcexatt(_0, _1, _2, _3, _4, _5) -> i32);
    count_import!(wld_setguildattitude(_0, _1, _2));
    count_import!(wld_getguildattitude(_0, _1) -> i32);
    count_import!(wld_ismobavailable(_0, _1) -> i32);
    count_import!(wld_isfpavailable(_0, _1) -> i32);
    count_import!(wld_isnextfpavailable(_0, _1) -> i32);
    count_import!(npc_createspell(_0, _1));
    count_import!(npc_learnspell(_0, _1));
    count_import!(npc_hasitems(_0, _1) -> i32);
    count_import!(npc_giveitem(_0, _1, _2));
    count_import!(createinvitem(_0, _1));
    count_import!(createinvitems(_0, _1, _2));
    count_import!(npc_getinvitem(_0, _1) -> i32);
    count_import!(npc_getinvitembyslot(_0, _1, _2) -> i32);
    count_import!(npc_removeinvitem(_0, _1) -> i32);
    count_import!(npc_removeinvitems(_0, _1, _2) -> i32);
    count_import!(npc_clearinventory(_0));
    count_import!(npc_isinstate(_0, _1) -> i32);
    count_import!(npc_wasinstate(_0, _1) -> i32);
    count_import!(npc_isinroutine(_0, _1) -> i32);
    count_import!(ai_setnpcstostate(_0, _1, _2));
    count_import!(npc_hasdetectednpc(_0, _1) -> i32);
    count_import!(npc_setattitude(_0, _1));
    count_import!(npc_settempattitude(_0, _1));
    count_import!(npc_getattitude(_0, _1) -> i32);
    count_import!(npc_getpermattitude(_0, _1) -> i32);
    count_import!(npc_changeattribute(_0, _1, _2));
    count_import!(npc_getcomrades(_0) -> i32);
    count_import!(npc_isnear(_0, _1) -> i32);
    count_import!(npc_getdisttonpc(_0, _1) -> i32);
    count_import!(npc_getheighttonpc(_0, _1) -> i32);
    count_import!(npc_getheighttoitem(_0, _1) -> i32);
    count_import!(npc_getdisttoplayer(_0) -> i32);
    count_import!(npc_getdisttoitem(_0, _1) -> i32);
    count_import!(npc_getdisttowp(_0, _1) -> i32);
    count_import!(npc_canseenpc(_0, _1) -> i32);
    count_import!(npc_canseenpcfreelos(_0, _1) -> i32);
    count_import!(npc_canseeitem(_0, _1) -> i32);
    count_import!(npc_isplayer(_0) -> i32);
    count_import!(npc_knowsplayer(_0, _1) -> i32);
    count_import!(npc_setknowsplayer(_0, _1));
    count_import!(npc_isinfightmode(_0, _1) -> i32);
    count_import!(npc_isaiming(_0, _1) -> i32);
    count_import!(npc_startitemreactmodules(_0, _1, _2) -> i32);
    count_import!(npc_hasoffered(_0, _1, _2) -> i32);
    count_import!(equipitem(_0, _1));
    count_import!(npc_settofistmode(_0));
    count_import!(npc_settofightmode(_0, _1));
    count_import!(npc_isdead(_0) -> i32);
    count_import!(ai_startstate(_0, _1, _2, _3));
    count_import!(ai_continueroutine(_0));
    count_import!(npc_getstatetime(_0) -> i32);
    count_import!(npc_setstatetime(_0, _1));
    // count_import!(ai_output(_0, _1, _2));
    count_import!(ai_outputsvm(_0, _1, _2));
    count_import!(ai_outputsvm_overlay(_0, _1, _2));
    count_import!(ai_playcutscene(_0, _1));
    count_import!(ai_canseenpc(_0, _1, _2));
    count_import!(ai_wait(_0, _1));
    count_import!(ai_waitms(_0, _1));
    count_import!(ai_waittillend(_0, _1));
    count_import!(ai_aligntowp(_0));
    count_import!(ai_setwalkmode(_0, _1));
    count_import!(ai_playani(_0, _1));
    count_import!(ai_playanibs(_0, _1, _2));
    count_import!(ai_gotowp(_0, _1));
    count_import!(ai_teleport(_0, _1));
    count_import!(ai_gotoitem(_0, _1));
    count_import!(ai_gotonpc(_0, _1));
    count_import!(ai_aligntofp(_0));
    count_import!(ai_gotofp(_0, _1));
    count_import!(npc_isonfp(_0, _1) -> i32);
    count_import!(ai_gotonextfp(_0, _1));
    count_import!(ai_gotosound(_0));
    count_import!(ai_takeitem(_0, _1));
    count_import!(ai_dropitem(_0, _1));
    count_import!(ai_drawweapon(_0));
    count_import!(ai_removeweapon(_0));
    count_import!(npc_hasrangedweaponwithammo(_0) -> i32);
    count_import!(npc_hasequippedweapon(_0) -> i32);
    count_import!(npc_hasequippedmeleeweapon(_0) -> i32);
    count_import!(npc_hasequippedrangedweapon(_0) -> i32);
    count_import!(npc_hasequippedarmor(_0) -> i32);
    count_import!(npc_hasreadiedweapon(_0) -> i32);
    count_import!(npc_hasreadiedmeleeweapon(_0) -> i32);
    count_import!(npc_hasreadiedrangedweapon(_0) -> i32);
    count_import!(npc_gettrueguild(_0) -> i32);
    count_import!(npc_settrueguild(_0, _1) -> i32);
    count_import!(npc_arewestronger(_0, _1) -> i32);
    count_import!(npc_gettarget(_0) -> i32);
    count_import!(npc_getnexttarget(_0) -> i32);
    count_import!(npc_getlookattarget(_0) -> i32);
    count_import!(npc_isnexttargetavailable(_0) -> i32);
    count_import!(npc_settarget(_0, _1));
    count_import!(npc_getbodystate(_0) -> i32);
    count_import!(npc_hasbodyflag(_0, _1) -> i32);
    count_import!(npc_isincutscene(_0) -> i32);
    count_import!(npc_isvoiceactive(_0) -> i32);
    count_import!(npc_stopani(_0, _1));
    count_import!(npc_getdetectedmob(_0) -> i32);
    count_import!(npc_playani(_0, _1));
    count_import!(npc_isdetectedmobownedbynpc(_0, _1) -> i32);
    count_import!(npc_isdetectedmobownedbyguild(_0, _1) -> i32);
    count_import!(wld_getmobstate(_0, _1) -> i32);
    count_import!(npc_clearaiqueue(_0));
    count_import!(infomanager_hasfinished() -> i32);
    // count_import!(ai_stopprocessinfos(_0));
    count_import!(ai_processinfos(_0));
    count_import!(info_addchoice(_0, _1, _2));
    count_import!(info_clearchoices(_0));
    count_import!(npc_knowsinfo(_0, _1) -> i32);
    count_import!(npc_checkinfo(_0, _1) -> i32);
    count_import!(npc_giveinfo(_0, _1) -> i32);
    count_import!(npc_gettalentskill(_0, _1) -> i32);
    count_import!(npc_gettalentvalue(_0, _1) -> i32);
    count_import!(npc_settalentskill(_0, _1, _2));
    count_import!(npc_settalentvalue(_0, _1, _2));
    count_import!(tal_configure(_0, _1));
    count_import!(npc_getnearestwp(_0) -> i32);
    count_import!(npc_getnextwp(_0) -> i32);
    count_import!(npc_iswayblocked(_0) -> i32);
    count_import!(npc_getreadiedweapon(_0) -> i32);
    count_import!(npc_getequippedmeleeweapon(_0) -> i32);
    count_import!(npc_getequippedrangedweapon(_0) -> i32);
    count_import!(npc_getequippedarmor(_0) -> i32);
    count_import!(ai_equipbestmeleeweapon(_0) -> i32);
    count_import!(ai_equipbestrangedweapon(_0) -> i32);
    count_import!(ai_equipbestarmor(_0));
    count_import!(ai_unequipweapons(_0));
    count_import!(ai_unequiparmor(_0));
    count_import!(ai_equiparmor(_0, _1));
    count_import!(ai_readymeleeweapon(_0));
    count_import!(ai_readyrangedweapon(_0));
    count_import!(ai_turnaway(_0, _1));
    count_import!(ai_turntonpc(_0, _1));
    count_import!(ai_whirlaround(_0, _1));
    count_import!(ai_whirlaroundtosource(_0));
    count_import!(ai_turntosound(_0));
    count_import!(ai_quicklook(_0, _1));
    count_import!(ai_lookat(_0, _1));
    count_import!(ai_lookatnpc(_0, _1));
    count_import!(ai_stoplookat(_0));
    count_import!(ai_pointat(_0, _1));
    count_import!(ai_pointatnpc(_0, _1));
    count_import!(ai_stoppointat(_0));
    count_import!(ai_standup(_0));
    count_import!(ai_standupquick(_0));
    count_import!(ai_flee(_0));
    count_import!(ai_aimat(_0, _1));
    count_import!(ai_stopaim(_0));
    count_import!(ai_shootat(_0, _1));
    count_import!(ai_defend(_0));
    count_import!(ai_combatreacttodamage(_0));
    count_import!(ai_readyspell(_0, _1, _2));
    count_import!(ai_unreadyspell(_0));
    count_import!(npc_hasspell(_0, _1) -> i32);
    count_import!(ai_attack(_0));
    count_import!(ai_finishingmove(_0, _1));
    count_import!(ai_dodge(_0));
    count_import!(ai_useitem(_0, _1));
    count_import!(ai_useitemtostate(_0, _1, _2));
    count_import!(npc_refusetalk(_0) -> i32);
    count_import!(npc_setrefusetalk(_0, _1));
    count_import!(ai_ask(_0, _1, _2));
    count_import!(ai_asktext(_0, _1, _2, _3, _4));
    count_import!(ai_waitforquestion(_0, _1));
    count_import!(hlp_isitem(_0, _1) -> i32);
    count_import!(hlp_getinstanceid(_0) -> i32);
    count_import!(hlp_random(_0) -> i32);
    count_import!(hlp_strcmp(_0, _1) -> i32);
    count_import!(hlp_getnpc(_0) -> i32);
    count_import!(hlp_isvalidnpc(_0) -> i32);
    count_import!(hlp_isvaliditem(_0) -> i32);
    count_import!(ai_snd_play(_0, _1));
    count_import!(ai_snd_play3d(_0, _1, _2));
    count_import!(snd_play(_0));
    count_import!(snd_play3d(_0, _1));
    count_import!(snd_getdisttosource(_0) -> i32);
    count_import!(snd_issourcenpc(_0) -> i32);
    count_import!(snd_issourceitem(_0) -> i32);
    count_import!(npc_canseesource(_0) -> i32);
    count_import!(mis_getstatus(_0) -> i32);
    count_import!(mis_setstatus(_0, _1));
    count_import!(mis_ontime(_0) -> i32);
    count_import!(log_createtopic(_0, _1));
    count_import!(log_addentry(_0, _1));
    count_import!(log_settopicstatus(_0, _1));
    count_import!(mis_addmissionentry(_0, _1));
    count_import!(mis_removemission(_0));
    count_import!(npc_checkavailablemission(_0, _1, _2) -> i32);
    count_import!(npc_checkrunningmission(_0, _1) -> i32);
    count_import!(npc_isdrawingweapon(_0) -> i32);
    count_import!(npc_isdrawingspell(_0) -> i32);
    count_import!(npc_checkoffermission(_0, _1) -> i32);
    // count_import!(mdl_setvisual(_0, _1));
    // count_import!(mdl_setvisualbody(_0, _1, _2, _3, _4, _5, _6, _7));
    count_import!(mdl_applyoverlaymds(_0, _1));
    count_import!(mdl_applyoverlaymdstimed(_0, _1, _2));
    count_import!(mdl_removeoverlaymds(_0, _1));
    count_import!(mdl_applyrandomani(_0, _1, _2));
    count_import!(mdl_applyrandomanifreq(_0, _1, _2));
    count_import!(mdl_setmodelscale(_0, _1, _2, _3));
    count_import!(mdl_setmodelfatness(_0, _1));
    count_import!(mdl_startfaceani(_0, _1, _2, _3));
    count_import!(mdl_applyrandomfaceani(_0, _1, _2, _3, _4, _5, _6));
    count_import!(wld_istime(_0, _1, _2, _3) -> i32);
    count_import!(wld_getday() -> i32);
    count_import!(wld_israining() -> i32);
    count_import!(wld_settime(_0, _1));
    // count_import!(wld_insertnpc(_0, _1));
    count_import!(wld_spawnnpcrange(_0, _1, _2, _3));
    count_import!(wld_playeffect(_0, _1, _2, _3, _4, _5, _6));
    count_import!(wld_stopeffect(_0));
    count_import!(ai_playfx(_0, _1, _2));
    count_import!(ai_stopfx(_0, _1));
    count_import!(wld_removenpc(_0));
    count_import!(wld_insertnpcandrespawn(_0, _1, _2));
    // count_import!(wld_insertitem(_0, _1));
    count_import!(wld_insertobject(_0, _1));
    count_import!(wld_removeitem(_0) -> i32);
    count_import!(wld_exchangeguildattitudes(_0));
    count_import!(wld_setobjectroutine(_0, _1, _2, _3));
    count_import!(wld_setmobroutine(_0, _1, _2, _3));
    count_import!(wld_sendtrigger(_0));
    count_import!(wld_senduntrigger(_0));
    count_import!(npc_ownedbynpc(_0, _1) -> i32);
    count_import!(npc_ownedbyguild(_0, _1) -> i32);
    count_import!(ai_takemob(_0, _1));
    count_import!(ai_dropmob(_0));
    count_import!(ai_usemob(_0, _1, _2) -> i32);
    count_import!(mob_createitems(_0, _1, _2));
    count_import!(mob_hasitems(_0, _1) -> i32);
    count_import!(doc_create() -> i32);
    count_import!(doc_createmap() -> i32);
    count_import!(doc_setpages(_0, _1));
    count_import!(doc_setpage(_0, _1, _2, _3));
    count_import!(doc_setfont(_0, _1, _2));
    count_import!(doc_setlevel(_0, _1));
    count_import!(doc_setlevelcoords(_0, _1, _2, _3, _4));
    count_import!(doc_setmargins(_0, _1, _2, _3, _4, _5, _6));
    count_import!(doc_printline(_0, _1, _2));
    count_import!(doc_printlines(_0, _1, _2));
    count_import!(doc_show(_0));
    count_import!(doc_open(_0));
    count_import!(doc_font(_0));
    count_import!(doc_print(_0));
    count_import!(doc_mapcoordinates(_0, _1, _2, _3, _4, _5, _6, _7, _8));
    count_import!(ta(_0, _1, _2, _3, _4));
    // count_import!(ta_min(_0, _1, _2, _3, _4, _5, _6));
    count_import!(ta_cs(_0, _1, _2));
    count_import!(ta_beginoverlay(_0));
    count_import!(ta_endoverlay(_0));
    count_import!(ta_removeoverlay(_0));
    count_import!(npc_exchangeroutine(_0, _1));
    count_import!(rtn_exchange(_0, _1));
    count_import!(perc_setrange(_0, _1));
    count_import!(npc_setperctime(_0, _1));
    count_import!(npc_percenable(_0, _1, _2));
    count_import!(npc_percdisable(_0, _1));
    count_import!(npc_sendpassiveperc(_0, _1, _2, _3));
    count_import!(npc_sendsingleperc(_0, _1, _2));
    count_import!(npc_perceiveall(_0));
    count_import!(wld_assignroomtoguild(_0, _1));
    count_import!(wld_assignroomtonpc(_0, _1));
    count_import!(wld_getplayerportalowner() -> i32);
    count_import!(wld_getplayerportalguild() -> i32);
    count_import!(wld_getformerplayerportalowner() -> i32);
    count_import!(wld_getformerplayerportalguild() -> i32);
    count_import!(npc_isplayerinmyroom(_0) -> i32);
    count_import!(npc_isinplayersroom(_0) -> i32);
    count_import!(npc_wasplayerinmyroom(_0) -> i32);
    count_import!(npc_getportalowner(_0) -> i32);
    count_import!(npc_getportalguild(_0) -> i32);
    count_import!(hlp_cutsceneplayed(_0) -> i32);
    count_import!(game_initgerman() -> i32);
    count_import!(game_initenglish() -> i32);
    count_import!(game_initengintl() -> i32);
}
