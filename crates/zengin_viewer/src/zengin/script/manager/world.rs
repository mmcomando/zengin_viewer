use std::path::Path;

use bevy::log::{info, warn};

use super::ScriptManager;

impl ScriptManager {
    /// Runs the script entry points of a world load. `clean_spawn` is a fresh
    /// world without saved state, only then the startup functions run.
    pub fn load_world(&mut self, world_name: &str, clean_spawn: bool) {
        let Some(script_name) = world_script_name(world_name) else {
            warn!("No script name for world({world_name})");
            return;
        };

        self.reset_world_state();

        if clean_spawn && !self.global_startup_done {
            self.call_optional_function("startup_global");
            self.global_startup_done = true;
        }
        if clean_spawn {
            self.call_optional_function(&format!("startup_{script_name}"));
        }
        self.call_optional_function("init_global");
        self.call_optional_function(&format!("init_{script_name}"));

        self.call_npc_routines();
    }

    /// Drops the per-world script bookkeeping so reloading a world does not
    /// accumulate routine entries, mob routines or pending spawns.
    fn reset_world_state(&mut self) {
        let data = self.store.data_mut();
        data.pending.spawn_items.clear();
        data.pending.spawn_npcs.clear();
        data.loaded_state.mob_routines.clear();
        data.guild_attitudes.clear();
        for npc in data.loaded_state.npc_instances.values_mut() {
            npc.routine_entries.clear();
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
