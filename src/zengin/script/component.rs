use crate::zengin::script::state::{DialogLine, LoadedState};
use crate::zengin::{common::gothic2_dir, script::main::WasmExecutor};
use bevy::ecs::resource::Resource;
use bevy::prelude::*;
use std::path::PathBuf;

pub struct GothicScriptsPlugin;

impl Plugin for GothicScriptsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(WorldScript::new());
        app.add_systems(Startup, spawn_entities);
    }
}

/// Component for configuring movement settings for a character controller.
#[derive(Resource)]
pub struct WorldScript {
    script: WasmExecutor,
}

impl WorldScript {
    pub fn get_loaded_state(&self) -> &LoadedState {
        let data = self.script.store.data();
        &data.loaded_state
    }
}

impl WorldScript {
    fn new() -> Self {
        let script_path = PathBuf::from(gothic2_dir() + "/_work/Data/Scripts/_compiled/GOTHIC.DAT");
        let cached_wasm_path = PathBuf::from("./gothic.script.wasm");
        let mut executor = WasmExecutor::from_file_cached(&script_path, &cached_wasm_path);

        println!("Call script initialize instances");
        executor.initialize_instances();
        let symbol_id = executor.get_symbol_id("init_newworld");
        println!("Call script init_newworld function");
        executor.call_function(symbol_id);
        let symbol_id = executor.get_symbol_id("startup_newworld");
        println!("Call script startup_newworld function");
        executor.call_function(symbol_id);
        println!("Call script npc routines");
        executor.call_npc_routines();
        println!("Call script load item instances");
        executor.load_item_instances();
        println!("Call script load npc instances");
        executor.load_npc_instances();
        println!("Call script load dialog options");
        executor.load_dialogs_information();

        Self { script: executor }
    }
}

impl WorldScript {
    pub fn clear_dialog_lines(&mut self) {
        self.script
            .store
            .data_mut()
            .dialog_state
            .dialog_lines
            .clear();
    }

    pub fn dialog_lines(&self) -> &[DialogLine] {
        &self.script.store.data().dialog_state.dialog_lines
    }

    pub fn call_dialog_sequence_function(&mut self, sequence_function_id: i32) {
        self.script
            .call_dialog_sequence_function(sequence_function_id);
    }
}

fn spawn_entities(_script: ResMut<WorldScript>) {
    // let data = script.script.store.data();
    // for spawn_npc in &data.loaded_state.spawn_npcs {
    //     //     let npc_id = *npc_id as u32;
    //     //     let string_id = *string_id as u32;
    //     // let symbol_name = executor.call_get_name(spawn_npc.npc_index);
    //     //     let waypoint_string = executor.call_get_name(string_id);

    //     // println!(
    //     //     "spawn_npc npc_index({}), way_point({})",
    //     //     spawn_npc.npc_index, spawn_npc.way_point
    //     // );
    // }

    // for spawn_item in &data.loaded_state.spawn_items {
    //     // println!(
    //     //     "spawn_item instance_name({}), way_point({})",
    //     //     spawn_item.instance_name, spawn_item.way_point
    //     // );
    // }
    // let name = script.script.call_get_name(1000);
    // println!("script name 0: {:?}", name);
}
