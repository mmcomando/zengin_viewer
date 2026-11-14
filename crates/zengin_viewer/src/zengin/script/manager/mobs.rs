//! Script-side mirror of the interactive mobs and their state hooks.

use bevy::log::{debug, warn};

use super::ScriptManager;
use crate::zengin::script::state::ScriptMob;
use crate::zengin::script::symbol_id::{FunctionId, InstanceId};

impl ScriptManager {
    /// Replaces the mob table. Keys must be lowercased schemes.
    pub fn set_mobs(&mut self, mobs: Vec<(String, ScriptMob)>) {
        let mut by_scheme = std::collections::HashMap::new();
        for (scheme, mob) in mobs {
            by_scheme.entry(scheme).or_insert_with(Vec::new).push(mob);
        }
        self.store.data_mut().mobs = by_scheme;
    }

    /// Runs the `<onStateFunc>_S<state>` hook of a mob when an npc reaches the
    /// mob's final state.
    pub fn call_mob_state_hook(&mut self, npc: InstanceId, on_state_func: &str, state: i32) {
        if on_state_func.is_empty() || state <= 0 {
            return;
        }
        let name = format!("{}_s{state}", on_state_func.to_lowercase());
        let Some(function) = self.mob_state_function(&name) else {
            return;
        };
        self.set_self(npc);
        self.set_instance(npc);
        if let Err(err) = self.try_call_function(function) {
            warn!("mob state function({name}) failed: {err:?}");
            return;
        }
        debug!("npc({npc}) ran mob state function({name})");
    }

    fn mob_state_function(&mut self, name: &str) -> Option<FunctionId> {
        let Some(symbol) = self.try_get_symbol_id(name) else {
            warn!("mob state function({name}) is not declared");
            return None;
        };
        let function = self.store.data().symbol_table.function(symbol.value());
        if function.is_none() {
            warn!("mob state function({name}) is not a function");
        }
        function
    }
}
