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

    /// Replaces the container contents table. Keys must be lowercased vob
    /// names. A name shared by several chests resolves to the first one.
    pub fn set_containers(&mut self, containers: Vec<(String, Vec<(InstanceId, i32)>)>) {
        let mut by_name = std::collections::HashMap::new();
        for (name, items) in containers {
            by_name.entry(name).or_insert(items);
        }
        self.store.data_mut().containers = by_name;
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

    /// Checks a mob's `useWithItem` against the npc's inventory and runs the
    /// `player_mob_missing_item` refusal when the item is absent. Returns
    /// `true` when the interaction was refused.
    pub fn refuse_missing_mob_item(&mut self, npc: InstanceId, use_with_item: &str) -> bool {
        if use_with_item.is_empty() {
            return false;
        }
        let Some(item) = self.symbol_instance(&use_with_item.to_lowercase()) else {
            warn!("mob useWithItem({use_with_item}) is not a script instance");
            return false;
        };
        if self
            .inventory(npc)
            .iter()
            .any(|(held, count)| *held == item && *count > 0)
        {
            return false;
        }
        let Some(function) = self.try_get_function_id("player_mob_missing_item") else {
            warn!(
                "mob useWithItem({use_with_item}) is missing but player_mob_missing_item is not declared"
            );
            return true;
        };
        self.set_self(npc);
        self.set_instance(npc);
        if let Err(err) = self.try_call_function(function) {
            warn!("player_mob_missing_item npc({npc}) failed: {err:?}");
        }
        true
    }

    /// Runs a mob `conditionFunc` with the user bound as `self`. An absent
    /// function allows the interaction; a missing or failing function does not.
    pub fn mob_condition_allows(&mut self, npc: InstanceId, condition_func: &str) -> bool {
        if condition_func.is_empty() {
            return true;
        }
        let name = condition_func.to_lowercase();
        let Some(symbol) = self.try_get_symbol_id(&name) else {
            warn!("mob condition function({name}) is not declared");
            return false;
        };
        let Some(function) = self.store.data().symbol_table.function(symbol.value()) else {
            warn!("mob condition function({name}) is not a function");
            return false;
        };
        self.set_self(npc);
        self.set_instance(npc);
        match self.try_call_function_bool(function) {
            Ok(allowed) => allowed,
            Err(err) => {
                warn!("mob condition function({name}) failed: {err:?}");
                false
            }
        }
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
