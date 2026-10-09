use crate::zengin::script::state::{DialogLine, DialogRoot, LoadedState};
use crate::zengin::{common::gothic2_dir, script::main::WasmExecutor};
use bevy::ecs::resource::Resource;
use bevy::prelude::*;
use std::collections::HashMap;
use std::path::PathBuf;
use zen_kit_rs::cutscene_library::CutsceneLibrary;
use zen_kit_rs::misc::TextEncoding;
use zen_kit_rs::stream::Read;

pub struct GothicScriptsPlugin;

impl Plugin for GothicScriptsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(WorldScript::new());
        app.insert_resource(load_dialog_texts());
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

    pub fn clear_sub_dialog_choices(&mut self) {
        self.script
            .store
            .data_mut()
            .dialog_state
            .sub_dialog_choices
            .clear();
    }

    pub fn sub_dialog_choices(&self) -> &[DialogRoot] {
        &self.script.store.data().dialog_state.sub_dialog_choices
    }

    pub fn close_dialog(&self) -> bool {
        self.script.store.data().dialog_state.close_dialog
    }

    pub fn call_dialog_sequence_function(&mut self, sequence_function_id: i32) {
        self.script.store.data_mut().dialog_state.close_dialog = false;
        self.script
            .call_dialog_sequence_function(sequence_function_id);
    }

    pub fn call_get_name(&mut self, symbol_id: u32) -> String {
        self.script.call_get_name(symbol_id)
    }

    /// Executes the routine state function of the npc instance, returning the
    /// animation names the script requested via `AI_PlayAni(BS)`.
    pub fn call_routine_state_function(
        &mut self,
        npc_instance_id: i32,
        state_fn: u32,
    ) -> Vec<String> {
        self.script
            .call_routine_state_function(npc_instance_id as u32, state_fn)
    }

    /// Evaluates the condition function of every root with the npc instance
    /// set, dropping roots whose condition returns false
    pub fn filter_dialog_roots(&mut self, npc_instance_id: i32, roots: &mut Vec<DialogRoot>) {
        self.script.filter_dialog_roots(npc_instance_id, roots)
    }
}

/// Real texts of dialog lines loaded from Ou.bin, keyed by Ou.bin block name.
/// Script `AI_Output` references the blocks by the string literal passed to it.
#[derive(Resource, Default)]
pub struct DialogTexts {
    texts: HashMap<String, String>,
}

impl DialogTexts {
    pub fn get(&self, block_name: &str) -> Option<&String> {
        self.texts.get(block_name)
    }
}

fn load_dialog_texts() -> DialogTexts {
    let path = gothic2_dir() + "/_work/Data/Scripts/CONTENT/CUTSCENE/Ou.bin";
    let Some(bytes) = std::fs::read(&path).ok() else {
        println!("Failed to read cutscene library from {path}, dialog lines will miss real texts");
        return DialogTexts::default();
    };
    let read = Read::from_slice(&bytes).expect("failed to create reader for Ou.bin");
    let Some(library) = CutsceneLibrary::load_with_encoding(&read, TextEncoding::Windows1250)
    else {
        println!("Failed to load cutscene library from {path}, dialog lines will miss real texts");
        return DialogTexts::default();
    };

    let block_count = library.block_count();
    let blocks = library.blocks();
    let mut texts = HashMap::with_capacity(blocks.len());
    let mut blocks_without_message = 0;
    for block in blocks {
        let Some(message) = block.message() else {
            blocks_without_message += 1;
            continue;
        };
        texts.insert(block.name().to_lowercase(), message.text());
    }

    println!(
        "Loaded dialog texts from Ou.bin: blocks({}), with_text({}), blocks_without_message({})",
        block_count,
        texts.len(),
        blocks_without_message
    );

    DialogTexts { texts }
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
