use std::path::Path;

use bevy::log::{info, warn};

use crate::zengin::script::journal::JournalManager;
use crate::zengin::script::state::{LoadedState, PendingChanges, ScriptRandom};

use super::ScriptManager;

impl ScriptManager {
    /// Rebuilds every piece of per-world script state in place so a world can
    /// be spawned again without recreating the vm. Symbol metadata and strings
    /// are static, only values and runtime state are reset.
    pub fn restart_world_state(&mut self) {
        info!("Restart world script state");
        self.reset_symbol_values();

        {
            let data = self.store.data_mut();
            data.loaded_state = LoadedState::default();
            data.pending = PendingChanges::default();
            data.messages.clear();
            data.close_dialog = false;
            data.player_instance = None;
            data.journal = JournalManager::default();
            data.guild_attitudes.clear();
            data.way_points.clear();
            data.rng = ScriptRandom::default();
        }
        // `startup_global` writes globals which were just reset, it has to run
        // again on the next world load.
        self.global_startup_done = false;

        self.initialize_instances();
        self.load_item_instances();
        self.load_npc_instances();
        self.load_dialogs_information();
        self.clear_npc_changes();
        self.clear_item_changes();
    }

    /// Runs the script entry points of a world load. `clean_spawn` is a fresh
    /// world without saved state, only then the startup functions run.
    pub fn load_world(&mut self, world_name: &str, clean_spawn: bool) {
        self.reset_world_state();
        if !self.run_world_entry_points(world_name, clean_spawn) {
            return;
        }
        self.call_npc_routines();
        // Initial values are seeded on npc spawn, do not replay them as changes.
        self.clear_npc_changes();
    }

    /// Runs only the world script entry points (`startup_*` on a clean spawn,
    /// then `init_*`). The save restore reuses this without the per-world reset.
    pub(super) fn run_world_entry_points(&mut self, world_name: &str, clean_spawn: bool) -> bool {
        let Some(script_name) = world_script_name(world_name) else {
            warn!("No script name for world({world_name})");
            return false;
        };

        if clean_spawn && !self.global_startup_done {
            self.call_optional_function("startup_global");
            self.global_startup_done = true;
        }
        if clean_spawn {
            self.call_optional_function(&format!("startup_{script_name}"));
        }
        self.call_optional_function("init_global");
        self.call_optional_function(&format!("init_{script_name}"));
        true
    }

    /// Drops the per-world script bookkeeping so reloading a world does not
    /// accumulate routine entries, mob routines or pending spawns.
    fn reset_world_state(&mut self) {
        let data = self.store.data_mut();
        data.messages.spawn_items.clear();
        data.messages.spawn_npcs.clear();
        data.loaded_state.mob_routines.clear();
        data.guild_attitudes.clear();
        for npc in data.loaded_state.npc_instances.values() {
            npc.write()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .routine
                .entries
                .clear();
        }
    }

    fn call_optional_function(&mut self, name: &str) {
        let Some(function) = self.try_get_function_id(name) else {
            warn!("Script function({name}) is missing, skipped");
            return;
        };
        info!("Call script {name} function");
        self.call_function(function);
    }
}

/// Script entry point suffix of a world is its `.ZEN` file stem, lowercase.
fn world_script_name(world_name: &str) -> Option<String> {
    let stem = Path::new(world_name).file_stem()?.to_str()?;
    Some(stem.to_lowercase())
}
