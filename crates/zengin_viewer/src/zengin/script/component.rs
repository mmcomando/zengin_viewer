use crate::zengin::GameNpc;
use crate::zengin::script::string_value::StringCanon;
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
        app.insert_resource(script);
        app.insert_resource(load_dialog_texts());
        app.add_systems(Update, update_script_npc_state);
    }
}

/// Keeps the npc world positions in sync with the world. Uses `GlobalTransform`
/// so npcs that are parented to something are still tracked at their world
/// position; the player visual is a `GameNpc` child and is included. Every npc
/// position is cleared first, so a removed npc drops out and counts as dead.
fn update_script_npc_state(npcs: Query<(&GameNpc, &GlobalTransform)>, script: Res<ScriptResource>) {
    let mut script = script.lock();
    let instances = script.npc_instances_mut();
    for instance in instances.values_mut() {
        instance.position = None;
    }
    for (npc, transform) in &npcs {
        instances.entry(npc.npc_instance_id).or_default().position = Some(transform.translation());
    }
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

/// Loads and initializes the compiled world script. Straight-line sequence of
/// script calls; `#[allow]` because bevy's `info!` macros expand to branching
/// code, which pushes clippy's cognitive complexity far above the real one.
#[allow(clippy::cognitive_complexity)]
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
    texts: HashMap<StringCanon, String>,
}

impl DialogTexts {
    pub fn get(&self, block_name: &StringCanon) -> Option<&String> {
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
        texts.insert(StringCanon::new(block.name()), message.text());
    }

    DialogTexts { texts }
}
