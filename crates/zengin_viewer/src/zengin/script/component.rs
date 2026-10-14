use crate::zengin::GameNpc;
use crate::zengin::script::state::NpcLiveState;
use crate::zengin::{common::gothic2_dir, script::manager::ScriptManager};
use bevy::ecs::resource::Resource;
use bevy::log::info;
use bevy::prelude::*;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use zen_kit_rs::cutscene_library::CutsceneLibrary;
use zen_kit_rs::misc::TextEncoding;
use zen_kit_rs::stream::Read;

pub struct GothicScriptsPlugin;

impl Plugin for GothicScriptsPlugin {
    fn build(&self, app: &mut App) {
        let script = ScriptResource::new();
        app.insert_resource(ScriptNpcState(script.lock().npc_live_state()));
        app.insert_resource(script);
        app.insert_resource(load_dialog_texts());
        app.add_systems(Startup, spawn_entities);
        app.add_systems(Update, update_script_npc_state);
    }
}

/// Shares the live npc facts with the script host.
#[derive(Resource)]
pub struct ScriptNpcState(pub Arc<Mutex<NpcLiveState>>);

/// Keeps the shared npc positions in sync with the world. Uses `GlobalTransform`
/// so npcs that are parented to something are still tracked at their world
/// position; the player visual is a `GameNpc` child and is included. The map is
/// rebuilt every frame, so a removed npc drops out and counts as dead.
fn update_script_npc_state(npcs: Query<(&GameNpc, &GlobalTransform)>, state: Res<ScriptNpcState>) {
    let mut positions = HashMap::new();
    for (npc, transform) in &npcs {
        positions.insert(npc.npc_instance_id, transform.translation());
    }

    let Ok(mut live) = state.0.lock() else {
        return;
    };
    live.positions = positions;
}

/// Bevy resource holding the world script vm behind `Arc<Mutex>` so it can be
/// called from any place in the code: clones share the same vm, the script is
/// accessed by locking it instead of being threaded through function arguments
#[derive(Resource, Clone)]
pub struct ScriptResource(Arc<Mutex<ScriptManager>>);

impl ScriptResource {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(load_gothic_script_manager())))
    }

    pub fn lock(&self) -> MutexGuard<'_, ScriptManager> {
        self.0.lock().unwrap()
    }
}

fn load_gothic_script_manager() -> ScriptManager {
    let script_path = PathBuf::from(gothic2_dir() + "/_work/Data/Scripts/_compiled/GOTHIC.DAT");
    let cached_wasm_path = PathBuf::from("./gothic.script.wasm");
    let mut script = ScriptManager::from_file_cached(&script_path, &cached_wasm_path);

    // Load the symbol table before any script runs, host calls made during
    // initialization already verify typed symbol ids
    info!("Load script symbol table");
    script.load_symbol_table();

    info!("Call script initialize instances");
    script.initialize_instances();
    let init_function_id = script.get_function_id("init_newworld");
    info!("Call script init_newworld function");
    script.call_function(init_function_id);
    let startup_function_id = script.get_function_id("startup_newworld");
    info!("Call script startup_newworld function");
    script.call_function(startup_function_id);
    info!("Call script npc routines");
    script.call_npc_routines();
    info!("Call script load item instances");
    script.load_item_instances();
    info!("Call script load npc instances");
    script.load_npc_instances();
    info!("Call script load dialog options");
    script.load_dialogs_information();

    script
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
        error!("Failed to read cutscene library from {path}, dialog lines will miss real texts");
        return DialogTexts::default();
    };
    let read = Read::from_slice(&bytes).expect("failed to create reader for Ou.bin");
    let Some(library) = CutsceneLibrary::load_with_encoding(&read, TextEncoding::Windows1250)
    else {
        println!("Failed to load cutscene library from {path}, dialog lines will miss real texts");
        return DialogTexts::default();
    };

    let blocks = library.blocks();
    let mut texts = HashMap::with_capacity(blocks.len());
    for block in blocks {
        let Some(message) = block.message() else {
            continue;
        };
        texts.insert(block.name().to_lowercase(), message.text());
    }

    DialogTexts { texts }
}

fn spawn_entities() {
    // TODO more modular spawning of object instead of listing objects in spawn_world function
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
