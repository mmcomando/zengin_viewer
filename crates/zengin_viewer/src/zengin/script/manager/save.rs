//! Snapshot and restore of the script-owned save state.
//!
//! Snapshotting reads the typed registry and the host runtime under one script
//! lock. Restoring rebuilds the registry in place (like the world restart) and
//! then applies the saved values; routine state is deliberately left to the
//! normal initialization path.

use std::collections::HashSet;
use std::sync::{Arc, PoisonError, RwLock};

use bevy::log::info;
use daedalus_typed::SymbolKind;

use crate::save::format::{SavedGuildAttitude, SavedValues, ScriptSnapshot};
use crate::zengin::{ItemHandle, NpcHandle};

use super::super::journal::JournalManager;
use super::super::state::{ItemRuntime, LoadedState, NpcRuntime, PendingChanges, ScriptRandom};
use super::super::symbol_id::InstanceId;

impl super::ScriptManager {
    /// Snapshot of every piece of script state that is persisted across a save.
    pub fn snapshot_save(&self) -> ScriptSnapshot {
        let data = self.store.data();
        let typed = &data.typed;

        let mut globals = Vec::new();
        let mut instances = Vec::new();
        let mut sanitized_dynamic_strings = 0usize;
        for symbol_index in 0..typed.symbol_count() {
            let Ok(symbol) = u32::try_from(symbol_index) else {
                continue;
            };
            if typed.is_class_member(symbol) || typed.is_const(symbol) {
                continue;
            }
            if !typed.has_value_slot(symbol) {
                continue;
            }
            let Some(kind) = typed.kind(symbol) else {
                continue;
            };
            let Some(mut values) = typed.snapshot_values(symbol) else {
                continue;
            };
            for index in typed.string_word_indices(symbol) {
                let Ok(index) = usize::try_from(index) else {
                    continue;
                };
                let Some(value) = values.get_mut(index) else {
                    continue;
                };
                if data.symbol_table.is_dynamic_string_id(*value) {
                    *value = 0;
                    sanitized_dynamic_strings += 1;
                }
            }
            // Function parameter symbols and other empty slots have no state.
            if values.is_empty() {
                continue;
            }
            // Unchanged symbols are recreated by the load's initialization.
            if typed.matches_default(symbol, &values) {
                continue;
            }
            let saved = SavedValues {
                id: i32::try_from(symbol_index).unwrap_or_default(),
                name: typed.name(symbol).unwrap_or_default().to_string(),
                values,
            };
            if kind == SymbolKind::Instance {
                instances.push(saved);
            } else {
                globals.push(saved);
            }
        }
        if sanitized_dynamic_strings > 0 {
            info!("Save sanitized {sanitized_dynamic_strings} dynamic string ids to empty strings");
        }

        let mut npc_runtimes: Vec<(InstanceId, NpcRuntime)> = data
            .loaded_state
            .npc_instances
            .iter()
            .map(|(id, runtime)| {
                let runtime = runtime.read().unwrap_or_else(PoisonError::into_inner);
                (*id, runtime.clone())
            })
            .collect();
        npc_runtimes.sort_by_key(|(id, _)| id.value());

        let mut item_runtimes: Vec<(InstanceId, ItemRuntime)> = data
            .loaded_state
            .items_instances
            .iter()
            .map(|(id, runtime)| {
                let runtime = runtime.read().unwrap_or_else(PoisonError::into_inner);
                (*id, runtime.clone())
            })
            .collect();
        item_runtimes.sort_by_key(|(id, _)| id.value());

        let mut guild_attitudes: Vec<SavedGuildAttitude> = data
            .guild_attitudes
            .iter()
            .map(|((guild_a, guild_b), attitude)| SavedGuildAttitude {
                guild_a: *guild_a,
                guild_b: *guild_b,
                attitude: *attitude,
            })
            .collect();
        guild_attitudes.sort_by_key(|entry| (entry.guild_a, entry.guild_b));

        ScriptSnapshot {
            player_instance: data.player_instance,
            globals,
            instances,
            npc_runtimes,
            item_runtimes,
            journal: data.journal.topics().to_vec(),
            guild_attitudes,
            mob_routines: data.loaded_state.mob_routines.clone(),
        }
    }

    /// Rebuilds the per-world script state from a snapshot. The world loader
    /// must have parsed the ZEN and published waypoints already; they are kept.
    /// Returns the handles of the restored npc and item instances.
    pub fn restore_world_from_save(
        &mut self,
        world_name: &str,
        save: &ScriptSnapshot,
    ) -> (Vec<NpcHandle>, Vec<ItemHandle>) {
        info!("Restore world script state from save");
        self.reset_symbol_values();
        {
            let data = self.store.data_mut();
            data.loaded_state = LoadedState::default();
            data.pending = PendingChanges::default();
            data.close_dialog = false;
            data.player_instance = None;
            data.journal = JournalManager::default();
            data.guild_attitudes.clear();
            data.check_info.clear();
            // Waypoints were published by the world spawn before this call and
            // must stay available to the world entry points.
            data.rng = ScriptRandom::default();
        }

        self.apply_globals(&save.globals);
        self.initialize_instances();
        if !self.run_world_entry_points(world_name, false) {
            info!("No world entry points ran while restoring save state");
        }
        // Symbols omitted from the save are at their defaults at save time, so
        // undo any writes performed by initialization/`init_*`.
        self.reset_unsaved_symbols(save);
        // Saved state wins over initialization and `init_*` writes.
        self.apply_globals(&save.globals);
        self.apply_instances(&save.instances);
        self.restore_npc_runtimes(&save.npc_runtimes);
        self.restore_item_runtimes(&save.item_runtimes);
        self.journal_mut().replace_topics(save.journal.clone());
        self.restore_guild_attitudes(&save.guild_attitudes);
        self.store
            .data_mut()
            .loaded_state
            .mob_routines
            .clone_from(&save.mob_routines);
        // Do not restore `player_instance` here: the world spawn checks it to
        // decide whether the player entity still has to be created, and
        // `spawn_player` sets it to the resolved `PC_HERO` instance.

        // Startup results came from the save, do not run them on world travel.
        self.global_startup_done = true;
        self.clear_pending_changes();
        self.clear_npc_changes();
        self.clear_item_changes();

        (self.npc_handles(), self.item_handles())
    }

    fn reset_unsaved_symbols(&mut self, save: &ScriptSnapshot) {
        let saved: HashSet<i32> = save
            .globals
            .iter()
            .chain(&save.instances)
            .map(|values| values.id)
            .collect();
        let data = self.store.data_mut();
        let symbol_count = data.typed.symbol_count();
        for index in 0..symbol_count {
            let Ok(symbol) = u32::try_from(index) else {
                continue;
            };
            let Ok(saved_id) = i32::try_from(index) else {
                continue;
            };
            if saved.contains(&saved_id)
                || !data.typed.has_value_slot(symbol)
                || data.typed.is_class_member(symbol)
                || data.typed.is_const(symbol)
            {
                continue;
            }
            data.typed.reset_to_default(symbol);
        }
    }

    fn apply_globals(&mut self, values: &[SavedValues]) {
        for saved in values {
            let Ok(symbol) = u32::try_from(saved.id) else {
                continue;
            };
            let data = self.store.data_mut();
            if data.typed.is_const(symbol) {
                continue;
            }
            data.typed.restore_values(symbol, &saved.values);
        }
    }

    fn apply_instances(&mut self, values: &[SavedValues]) {
        for saved in values {
            let Ok(symbol) = u32::try_from(saved.id) else {
                continue;
            };
            let data = self.store.data_mut();
            if data.typed.is_const(symbol) {
                continue;
            }
            data.typed.restore_values(symbol, &saved.values);
        }
    }

    fn restore_npc_runtimes(&mut self, saved: &[(InstanceId, NpcRuntime)]) {
        let data = self.store.data_mut();
        for (instance, saved) in saved {
            let runtime = data
                .loaded_state
                .npc_instances
                .entry(*instance)
                .or_default()
                .clone();
            let mut npc = runtime.write().unwrap_or_else(PoisonError::into_inner);
            npc.visual = saved.visual.clone();
            npc.equipment = saved.equipment.clone();
            npc.dialog = saved.dialog.clone();
            npc.inventory = saved.inventory.clone();
            npc.talents = saved.talents.clone();
            npc.social = saved.social.clone();
            npc.live = saved.live.clone();
            // `npc.routine` is intentionally left as initialized.
        }
    }

    fn restore_item_runtimes(&mut self, saved: &[(InstanceId, ItemRuntime)]) {
        let data = self.store.data_mut();
        data.loaded_state.items_instances = saved
            .iter()
            .map(|(instance, runtime)| (*instance, Arc::new(RwLock::new(runtime.clone()))))
            .collect();
    }

    fn restore_guild_attitudes(&mut self, saved: &[SavedGuildAttitude]) {
        let data = self.store.data_mut();
        data.guild_attitudes.clear();
        for entry in saved {
            data.guild_attitudes
                .insert((entry.guild_a, entry.guild_b), entry.attitude);
        }
    }
}
