use std::sync::{PoisonError, RwLockWriteGuard};

use bevy::log::warn;
use daedalus_typed::{Item, Npc};

use crate::zengin::{ItemHandle, NpcHandle};

use super::super::bindings::exports::mmcomando::zengine_scripts::manage::SymbolKind;
use super::super::state::{NpcRuntime, RoutineFunctions};
use super::super::string_value::ShowString;
use super::super::symbol_id::{FunctionId, InstanceId, StringId, SymbolId};
use super::{ItemDisplayData, attribute_display_name};

impl super::ScriptManager {
    /// Clears the npc's routine entries, points `C_Npc.daily_routine` at the
    /// `Rtn_{name}_{npcId}` function and runs it, which re-fills the entries via
    /// `TA_Min` (`Npc_ExchangeRoutine`).
    /// The suffix is the `C_Npc.id` member, not the instance symbol id, and
    /// script symbol names are stored lowercased.
    pub fn exchange_routine(&mut self, npc: InstanceId, routine_name: &str) {
        let Some(npc_id) = self.npc_data(npc.value() as u32).map(|npc| npc.id) else {
            warn!(
                "No C_Npc data for npc({}), cannot exchange routine({routine_name})",
                npc.value()
            );
            return;
        };
        let symbol_name = format!("rtn_{}_{npc_id}", routine_name.to_lowercase());
        let Some(symbol) = self.try_get_symbol_id(&symbol_name) else {
            warn!(
                "No routine symbol({symbol_name}) for npc({}, C_Npc.id {npc_id})",
                npc.value()
            );
            return;
        };
        let Some(function) = self.symbol_table().function(symbol.value()) else {
            warn!(
                "Routine symbol({symbol_name}) is not callable for npc({})",
                npc.value()
            );
            return;
        };
        {
            // The exchanged function becomes the npc's daily routine, so a save
            // that persists the instance record keeps it across a load.
            self.store
                .data_mut()
                .typed
                .set_npc_daily_routine(npc.value() as u32, Some(FunctionId::from_symbol(symbol)));
            self.npc_state_mut(npc).routine.entries.clear();
        }
        self.set_self(npc);
        self.set_state_other(None);
        if let Err(error) = self.try_call_function(function) {
            warn!("Routine function({symbol_name}) failed: {error:?}");
        }
    }

    /// Writes `C_Npc.wp` from a raw script string id.
    pub fn set_npc_waypoint(&mut self, npc: InstanceId, waypoint_id: i32) {
        let Some(waypoint) = self.store.data().symbol_table.string(waypoint_id) else {
            return;
        };
        let is_empty = self
            .store
            .data()
            .symbol_table
            .string_content(waypoint_id)
            .is_none_or(|content| content.is_empty());
        if is_empty {
            return;
        }
        self.store
            .data_mut()
            .typed
            .set_npc_waypoint(npc.value() as u32, Some(waypoint));
    }

    pub fn set_instance(&mut self, instance_index: InstanceId) {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_set_instance(&mut self.store, instance_index.value())
            .unwrap();
    }

    /// Binds the given instance as `self` for subsequent script calls
    pub fn set_self(&mut self, instance_index: InstanceId) {
        self.write_well_known("self", instance_index);
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_set_self(&mut self.store, instance_index.value())
            .unwrap();
    }

    /// Binds the given instance as `other` for subsequent script calls
    pub fn set_other(&mut self, instance_index: InstanceId) {
        self.write_well_known("other", instance_index);
    }

    /// Clears the `other` global for subsequent script calls.
    pub fn clear_other(&mut self) {
        self.write_well_known_value("other", 0);
    }

    /// Binds the `other` global of a state hook call, clearing it when the state
    /// has no partner.
    pub fn set_state_other(&mut self, other: Option<InstanceId>) {
        match other {
            Some(other) => self.set_other(other),
            None => self.clear_other(),
        }
    }

    /// Binds the given instance as `victim` for subsequent script calls, `None`
    /// clears it.
    pub fn set_victim(&mut self, instance_index: Option<InstanceId>) {
        self.write_well_known_value("victim", instance_index.map_or(0, InstanceId::value));
    }

    /// Writes a well known instance variable into the host symbol store, the
    /// guest export only updates its cached instance id used by
    /// `SetInstance(self)`.
    pub(super) fn write_well_known(&mut self, name: &str, instance_index: InstanceId) {
        self.write_well_known_value(name, instance_index.value());
    }

    fn write_well_known_value(&mut self, name: &str, value: i32) {
        let well_known = self.store.data().well_known_symbols;
        let symbol = match name {
            "self" => well_known.self_symbol,
            "other" => well_known.other_symbol,
            "victim" => well_known.victim_symbol,
            "item" => well_known.item_symbol,
            "hero" => well_known.hero_symbol,
            _ => None,
        };
        let Some(symbol) = symbol else {
            return;
        };
        self.store
            .data_mut()
            .typed
            .write_symbol(symbol as u32, 0, value);
    }

    pub fn call_npc_routine(&mut self, instance_index: InstanceId) {
        let Some(routine) = self
            .npc_data(instance_index.value() as u32)
            .and_then(|npc| npc.daily_routine)
        else {
            return;
        };
        let Some(function) = self.symbol_table().function(routine.value()) else {
            return;
        };
        if let Err(err) = self.try_call_function(function) {
            println!("call_npc_routine({instance_index}) error: {err:?}");
        }
    }

    /// Display name of an npc from its `C_NPC` `name` member, `None` when the
    /// instance has no data or the name string is unset.
    pub fn npc_display_name(&mut self, npc: InstanceId) -> Option<String> {
        let name = self.npc_data(npc.value() as u32)?.name;
        self.string_by_id(name.map(StringId::value))
    }

    /// Display name of an item from its `C_ITEM` `name` member, `None` when
    /// the instance has no data or the name string is unset.
    pub fn item_display_name(&mut self, item: InstanceId) -> Option<String> {
        let name = self.item_data(item.value() as u32)?.name;
        self.string_by_id(name.map(StringId::value))
    }

    /// Name of an item for the game log: localized `C_ITEM.name`, warn and the
    /// raw instance id when the item has no name data.
    pub fn item_log_name(&mut self, item: InstanceId) -> String {
        if let Some(name) = self.item_display_name(item) {
            return name;
        }
        warn!(
            "Item({}) has no C_ITEM.name, displaying the raw instance id",
            item.value()
        );
        item.value().to_string()
    }

    fn npc_data(&self, instance: u32) -> Option<Npc> {
        self.store.data().typed.npc(instance)
    }

    /// `C_Npc.fight_tactic`, the `FAI_*` fighter tactic table index. `None` when
    /// the instance has no typed `C_Npc` data.
    pub fn npc_fight_tactic(&self, npc: InstanceId) -> Option<i32> {
        Some(self.npc_data(npc.value() as u32)?.fight_tactic)
    }

    pub(super) fn item_data(&self, instance: u32) -> Option<Item> {
        self.store.data().typed.item(instance)
    }

    /// Shared handle of an npc instance, created on demand. The typed `C_Npc`
    /// record comes from the registry, the runtime state from the loaded state.
    pub fn npc_handle(&mut self, instance: InstanceId) -> Option<NpcHandle> {
        let Some(typed) = self
            .store
            .data()
            .typed
            .instance(instance.value() as u32)
            .and_then(daedalus_typed::TypedInstance::npc)
            .cloned()
        else {
            warn!("No typed C_Npc data for instance({})", instance.value());
            return None;
        };
        let runtime = self
            .store
            .data_mut()
            .loaded_state
            .npc_instances
            .entry(instance)
            .or_default()
            .clone();
        Some(NpcHandle::new(instance, typed, runtime))
    }

    /// Handles of every npc instance currently known to the script.
    pub fn npc_handles(&mut self) -> Vec<NpcHandle> {
        let ids: Vec<InstanceId> = self
            .store
            .data()
            .loaded_state
            .npc_instances
            .keys()
            .copied()
            .collect();
        ids.into_iter()
            .filter_map(|instance| self.npc_handle(instance))
            .collect()
    }

    /// Shared handle of an item instance, created on demand.
    pub fn item_handle(&mut self, instance: InstanceId) -> Option<ItemHandle> {
        let Some(typed) = self
            .store
            .data()
            .typed
            .instance(instance.value() as u32)
            .and_then(daedalus_typed::TypedInstance::item)
            .cloned()
        else {
            warn!("No typed C_Item data for instance({})", instance.value());
            return None;
        };
        let runtime = self
            .store
            .data_mut()
            .loaded_state
            .items_instances
            .entry(instance)
            .or_default()
            .clone();
        Some(ItemHandle::new(instance, typed, runtime))
    }

    /// Handles of every item instance currently known to the script.
    pub fn item_handles(&mut self) -> Vec<ItemHandle> {
        let ids: Vec<InstanceId> = self
            .store
            .data()
            .loaded_state
            .items_instances
            .keys()
            .copied()
            .collect();
        ids.into_iter()
            .filter_map(|instance| self.item_handle(instance))
            .collect()
    }

    /// Re-resolves the runtime visual strings of an item from its typed
    /// `C_Item` record. Used after a script edit; does not mark the item again.
    pub fn refresh_item_runtime(&mut self, item: InstanceId) {
        let Some(data) = self.item_data(item.value() as u32) else {
            return;
        };
        let visual = data
            .visual
            .and_then(|id| self.string_by_id(Some(id.value())))
            .unwrap_or_default();
        let visual_change = data
            .visual_change
            .and_then(|id| self.string_by_id(Some(id.value())));
        let state = self
            .store
            .data_mut()
            .loaded_state
            .items_instances
            .entry(item)
            .or_default()
            .clone();
        let mut state = state.write().unwrap_or_else(PoisonError::into_inner);
        state.visual = visual;
        state.visual_change = visual_change;
    }

    /// Hitpoints and hitpoints max of an npc, read from the typed `C_Npc` model
    /// (`ATR_HITPOINTS` and `ATR_HITPOINTS_MAX`), `None` when the id is not an
    /// npc instance.
    #[cfg(test)]
    pub fn hitpoints(&mut self, npc: InstanceId) -> Option<(i32, i32)> {
        let npc = self.npc_data(npc.value() as u32)?;
        Some((npc.hitpoints, npc.hitpoints_max))
    }

    /// Changes the hitpoints of an npc by `delta` (damage is negative),
    /// clamped to `0..=hitpoints_max`. Leaves the change queued for the ECS
    /// bridge. Returns the new hitpoints, `None` when the npc has no typed
    /// instance data.
    pub fn change_hitpoints(&mut self, npc: InstanceId, delta: i32) -> Option<i32> {
        let handle = self.npc_handle(npc)?.typed().clone();
        let changed = {
            let mut data = handle.write().unwrap_or_else(PoisonError::into_inner);
            let changed = (data.hitpoints + delta).clamp(0, data.hitpoints_max);
            data.hitpoints = changed;
            changed
        };
        self.pending_changes().npc_changed.insert(npc);
        Some(changed)
    }

    /// Total damage of an item instance (`C_Item.damagetotal`).
    pub fn item_damage_total(&mut self, item: InstanceId) -> i32 {
        self.item_data(item.value() as u32)
            .map_or(0, |item| item.damage_total)
    }

    /// Display stats of an item instance for the inventory info panel.
    pub fn item_display_data(&mut self, item: InstanceId) -> ItemDisplayData {
        let Some(info) = self.item_data(item.value() as u32) else {
            return ItemDisplayData::default();
        };
        let requirements = info
            .cond_atr
            .into_iter()
            .zip(info.cond_value)
            .filter_map(|(attribute, value)| {
                let name = attribute_display_name(attribute)?;
                Some((name, value))
            })
            .collect();
        let info_lines = info
            .text
            .into_iter()
            .zip(info.count)
            .filter_map(|(text, count)| {
                let text = self.string_by_id(text.map(StringId::value))?;
                Some((text, count))
            })
            .collect();
        ItemDisplayData {
            damage_total: info.damage_total,
            value: info.value,
            description: self.string_by_id(info.description.map(StringId::value)),
            requirements,
            info_lines,
        }
    }

    /// Resolves the `_loop`/`_end` hooks of a routine state function, cached per
    /// start function. Hook symbols are the state symbol name with the suffix.
    pub fn resolve_routine_functions(&mut self, start: FunctionId) -> RoutineFunctions {
        if let Some(functions) = self.routine_functions.get(&start) {
            return *functions;
        }
        let start_name = self.call_get_name(start.symbol());
        let loop_fn = self.resolve_routine_hook(&start_name, "_loop");
        let end_fn = self.resolve_routine_hook(&start_name, "_end");
        let functions = RoutineFunctions {
            start,
            loop_fn,
            end_fn,
        };
        self.routine_functions.insert(start, functions);
        functions
    }

    fn resolve_routine_hook(&mut self, start_name: &str, suffix: &str) -> Option<FunctionId> {
        let name = format!("{start_name}{suffix}");
        let symbol = self.try_get_symbol_id(&name)?;
        self.store.data().symbol_table.function(symbol.value())
    }

    /// Runs the start hook of a routine state.
    pub fn call_routine_start(&mut self, npc: InstanceId, functions: RoutineFunctions) {
        {
            let mut instance = self.npc_state_mut(npc);
            instance.routine.state_time = 0;
            if let Some(previous) = instance.routine.current_state.replace(functions.start)
                && previous != functions.start
            {
                instance.routine.previous_state = Some(previous);
            }
        }
        self.set_self(npc);
        self.set_instance(npc);
        if let Err(err) = self.try_call_function(functions.start) {
            warn!(
                "Routine start instance({npc}) function({}) failed: {err:?}",
                functions.start.value()
            );
        }
    }

    /// Runs the loop hook of a routine state, `true` when it returned
    /// `LOOP_END`.
    pub fn call_routine_loop(&mut self, npc: InstanceId, functions: RoutineFunctions) -> bool {
        let Some(loop_fn) = functions.loop_fn else {
            return false;
        };
        self.set_self(npc);
        self.set_instance(npc);
        match self.try_call_function_bool(loop_fn) {
            Ok(ended) => ended,
            Err(err) => {
                warn!(
                    "Routine loop instance({npc}) function({}) failed: {err:?}",
                    loop_fn.value()
                );
                false
            }
        }
    }

    /// Runs the end hook of a routine state and clears its active state.
    pub fn call_routine_end(&mut self, npc: InstanceId, functions: RoutineFunctions) {
        if let Some(end_fn) = functions.end_fn {
            self.set_self(npc);
            self.set_instance(npc);
            if let Err(err) = self.try_call_function(end_fn) {
                warn!(
                    "Routine end instance({npc}) function({}) failed: {err:?}",
                    end_fn.value()
                );
            }
        }
        let mut instance = self.npc_state_mut(npc);
        instance.routine.previous_state = instance.routine.current_state.take();
        instance.routine.state_time = 0;
    }

    /// Sets the state time the next `NPC_GetStateTime` call reports.
    pub fn set_routine_state_time(&mut self, npc: InstanceId, seconds: i32) {
        self.routine_state_mut(npc).routine.state_time = seconds;
    }

    /// Current routine state time, in seconds.
    pub fn routine_state_time(&self, npc: InstanceId) -> Option<i32> {
        self.npc_state(npc).map(|npc| npc.routine.state_time)
    }

    /// Mutable routine runtime state written without marking the instance as
    /// changed, the loop and state-time systems touch it every frame.
    fn routine_state_mut(&mut self, npc: InstanceId) -> RwLockWriteGuard<'_, NpcRuntime> {
        self.store
            .data_mut()
            .loaded_state
            .npc_instances
            .entry(npc)
            .or_default()
            .write()
            .unwrap_or_else(PoisonError::into_inner)
    }

    pub fn get_item_visual(&mut self, symbol_id: SymbolId) -> Option<String> {
        let visual = self.item_data(symbol_id.value() as u32)?.visual;
        self.string_by_id(visual.map(StringId::value))
    }
    pub fn get_item_visual_change(&mut self, symbol_id: SymbolId) -> Option<String> {
        let visual = self.item_data(symbol_id.value() as u32)?.visual_change;
        self.string_by_id(visual.map(StringId::value))
    }

    pub fn call_npc_routines(&mut self) {
        let npc_indices: Vec<_> = self
            .npc_instances()
            .iter()
            .filter(|(_, npc)| {
                npc.read()
                    .unwrap_or_else(PoisonError::into_inner)
                    .visual
                    .initialized
            })
            .map(|(index, _)| *index)
            .collect();
        for index in npc_indices {
            self.set_self(index);
            self.set_instance(index);
            self.call_npc_routine(index);
        }
    }

    pub fn load_item_instances(&mut self) {
        let symbols_num = self.get_all_symbols_num();

        let item_class_id = self.get_symbol_id("c_item");

        for index in 0..symbols_num {
            let Some(index) = SymbolId::new(index) else {
                continue;
            };
            let symbol_info = self.get_symbol_info(index);
            if symbol_info.kind != SymbolKind::Instance {
                continue;
            }
            // Kind was just verified, the checked conversion cannot fail
            let index = InstanceId::from_symbol(index);

            if !self.has_ancestor(&symbol_info, item_class_id) {
                continue;
            }
            if symbol_info.function.is_none() {
                // Instance doesn't have init code, so it won't have meaningful data
                continue;
            }
            let symbol_name = self.call_get_name(index.symbol());

            let Some(visual) = self.get_item_visual(index.symbol()) else {
                println!("No visual for item({})", symbol_name);
                continue;
            };

            let visual_change = self.get_item_visual_change(index.symbol());
            let mut entry = self.item_state_mut(index);
            entry.name = symbol_name;
            entry.visual = visual;
            entry.visual_change = visual_change;
        }
    }

    pub fn load_npc_instances(&mut self) {
        let indices: Vec<InstanceId> = self
            .npc_instances()
            .iter()
            .filter(|(_, npc)| {
                npc.read()
                    .unwrap_or_else(PoisonError::into_inner)
                    .visual
                    .initialized
            })
            .map(|(index, _)| *index)
            .collect();

        for index in indices {
            // Copy the raw visual body ids out before resolving them through
            // the guest (which needs `&mut self`).
            let Some(raw) = self.npc_instances().get(&index).map(|npc| {
                let npc = npc.read().unwrap_or_else(PoisonError::into_inner);
                (
                    npc.visual.body_model_id,
                    npc.visual.head_model_id,
                    npc.visual.hierarchy_id,
                    npc.visual.face_texture_index,
                    npc.visual.body_texture_index,
                )
            }) else {
                continue;
            };
            let (
                body_model_id,
                head_model_id,
                hierarchy_id,
                face_texture_index,
                body_texture_index,
            ) = raw;

            let Some(body_model) = body_model_id
                .and_then(|id| self.get_show_string(id))
                .map(ShowString::into_string)
            else {
                println!(
                    "Not found body model string for npc({index}), body_model({})",
                    body_model_id.map(StringId::value).unwrap_or_default()
                );
                continue;
            };
            // Empty strings are valid script values meaning "none" (e.g.
            // animals pass "" as the head model), filter them out here.
            let hierarchy = hierarchy_id
                .and_then(|id| self.get_show_string(id))
                .filter(|hierarchy| !hierarchy.string().is_empty());
            let head_model = head_model_id
                .and_then(|id| self.get_show_string(id))
                .filter(|head_model| !head_model.string().is_empty());

            // Sometimes there is model "Hum_Head_Babe." (dot at the end), engine or scripts bug?
            let head_model = head_model.map(|el| {
                let el = el.into_string();
                if !el.contains('.') {
                    return el;
                }
                el.replace('.', "")
            });

            let face_texture = if face_texture_index != 0 {
                Some(format!("HUM_HEAD_V{}_C0.TGA", face_texture_index))
            } else {
                None
            };

            let body_texture = if body_texture_index != 0 {
                Some(format!("HUM_BODY_NAKED_V{}_C0.TGA", body_texture_index))
            } else {
                None
            };

            let mut entry = self.npc_state_mut(index);
            entry.visual.body_model = body_model;
            entry.visual.body_texture = body_texture;
            entry.visual.head_model = head_model;
            entry.visual.face_texture = face_texture;
            entry.visual.hierarchy = hierarchy.map(ShowString::into_string);
        }
    }
}
