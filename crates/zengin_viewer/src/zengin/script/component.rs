use crate::game_state::AppState;
use crate::zengin::instance_index::InstanceIndex;
use crate::zengin::script::string_value::StringCanon;
use crate::zengin::{GameNpc, NpcHandle};
use crate::zengin::{common::gothic2_dir, script::manager::ScriptManager};
use bevy::ecs::resource::Resource;
use bevy::log::info;
use bevy::prelude::*;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use zen_kit_rs::cutscene_library::CutsceneLibrary;
use zen_kit_rs::misc::TextEncoding;
use zen_kit_rs::stream::Read;

pub struct GothicScriptsPlugin;

impl Plugin for GothicScriptsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(AppState::InGame),
            initialize_game_script.in_set(GameScriptLoaded),
        );
        app.add_systems(
            Update,
            update_script_npc_state.run_if(in_state(AppState::InGame)),
        );
    }
}

/// Marks the point in `OnEnter(InGame)` after which the game script and dialog
/// texts are available as resources.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct GameScriptLoaded;

/// Loads the game script and dialog texts when entering the game. This keeps
/// the main menu from paying for GOTHIC.DAT conversion or instance setup.
fn initialize_game_script(world: &mut World) {
    info!("Load game script");
    let mut script = load_gothic_script_manager();
    let npc_handles = script.npc_handles();
    let item_handles = script.item_handles();
    world.insert_resource(ScriptResource::from_manager(script));
    {
        let mut index = world.resource_mut::<InstanceIndex>();
        index.replace_npc_handles(npc_handles);
        index.replace_item_handles(item_handles);
    }
    world.insert_resource(load_dialog_texts());
}

/// Keeps the npc world positions in sync with the world. Uses `GlobalTransform`
/// so npcs that are parented to something are still tracked at their world
/// position; the player visual is a `GameNpc` child and is included. Only npcs
/// whose transform (or handle) changed are visited. Removal is not handled here,
/// dead state will be computed separately.
#[allow(clippy::type_complexity)]
fn update_script_npc_state(
    changed_npcs: Query<
        (&GameNpc, &GlobalTransform),
        Or<(Changed<GlobalTransform>, Changed<GameNpc>)>,
    >,
) {
    for (npc, transform) in &changed_npcs {
        set_npc_script_position(npc.handle(), Some(transform.translation()));
    }
}

fn set_npc_script_position(handle: &NpcHandle, position: Option<Vec3>) {
    handle
        .runtime()
        .write()
        .unwrap_or_else(PoisonError::into_inner)
        .live
        .position = position;
}

/// Bevy resource holding the world script vm behind `Arc<Mutex>` so it can be
/// called from any place in the code: clones share the same vm, the script is
/// accessed by locking it instead of being threaded through function arguments
#[derive(Resource, Clone)]
pub struct ScriptResource(Arc<Mutex<ScriptManager>>);

impl ScriptResource {
    pub fn from_manager(manager: ScriptManager) -> Self {
        Self(Arc::new(Mutex::new(manager)))
    }

    pub fn lock(&self) -> MutexGuard<'_, ScriptManager> {
        self.0.lock().unwrap()
    }
}

/// Loads the compiled script and its global instance data. World-specific
/// script entry points run on world load, see [`ScriptManager::load_world`].
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
    info!("Call script load item instances");
    script.load_item_instances();
    info!("Call script load npc instances");
    script.load_npc_instances();
    info!("Call script load dialog options");
    script.load_dialogs_information();

    // Initial values are seeded on npc spawn, do not replay them as changes.
    script.clear_npc_changes();
    script.clear_item_changes();

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
