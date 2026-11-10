use std::sync::PoisonError;

use bevy::log::warn;
use daedalus_typed::{Item, Npc};

use crate::zengin::{ItemHandle, NpcHandle};

use super::super::bindings::exports::mmcomando::zengine_scripts::manage::SymbolKind;
use super::super::string_value::{ShowString, StringCanon};
use super::super::symbol_id::{FunctionId, InstanceId, StringId, SymbolId};
use super::{ItemDisplayData, attribute_display_name};

impl super::ScriptManager {
    /// Clears the npc's routine entries and runs the `Rtn_{name}_{npcId}`
    /// function, which re-fills them via `TA_Min` (`Npc_ExchangeRoutine`).
    pub fn exchange_routine(&mut self, npc: InstanceId, routine_name: &str) {
        let symbol_name = format!("Rtn_{routine_name}_{}", npc.value());
        let Some(symbol) = self.try_get_symbol_id(&symbol_name) else {
            warn!("No routine symbol({symbol_name}) for npc({})", npc.value());
            return;
        };
        let Some(function) = self.symbol_table().function(symbol.value()) else {
            warn!(
                "Routine symbol({symbol_name}) is not callable for npc({})",
                npc.value()
            );
            return;
        };
        self.npc_state_mut(npc).routine.entries.clear();
        self.set_self(npc);
        if let Err(error) = self.try_call_function(function) {
            warn!("Routine function({symbol_name}) failed: {error:?}");
        }
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

    /// Writes a well known instance variable into the host symbol store, the
    /// guest export only updates its cached instance id used by
    /// `SetInstance(self)`.
    fn write_well_known(&mut self, name: &str, instance_index: InstanceId) {
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
            .write_symbol(symbol as u32, 0, instance_index.value());
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

    /// Executes the routine state function (`ZS_*`) for the npc instance with
    /// the instance set as `self`, followed by a single invocation of the
    /// state's `_loop` function when one exists, mirroring how the engine
    /// starts a routine state. Returns the animation names the script requested
    /// via `AI_PlayAni(BS)` in request order.
    pub fn call_routine_state_function(
        &mut self,
        npc_instance_id: InstanceId,
        state_fn: FunctionId,
    ) -> Vec<StringCanon> {
        self.npc_state_mut(npc_instance_id)
            .routine
            .played_anis
            .clear();
        // Track the active state so `Npc_IsInState`/`Npc_WasInState` answer
        // while the state function runs
        {
            let mut npc = self.npc_state_mut(npc_instance_id);
            if let Some(previous) = npc.routine.current_state.replace(state_fn)
                && previous != state_fn
            {
                npc.routine.previous_state = Some(previous);
            }
        }
        self.set_self(npc_instance_id);
        self.set_instance(npc_instance_id);
        if let Err(err) = self.try_call_function(state_fn) {
            println!(
                "Routine state function instance({npc_instance_id}) state_fn({}) error: {err:?}",
                state_fn.value()
            );
            return Vec::new();
        }

        if !self.routine_loop_functions.contains_key(&state_fn) {
            let loop_name = format!("{}_loop", self.call_get_name(state_fn.symbol()));
            let loop_fn = self
                .try_get_symbol_id(&loop_name)
                .and_then(|symbol| self.store.data().symbol_table.function(symbol.value()));
            self.routine_loop_functions.insert(state_fn, loop_fn);
        }
        let loop_fn = self.routine_loop_functions[&state_fn];
        if let Some(loop_fn) = loop_fn
            && let Err(err) = self.try_call_function_bool(loop_fn)
        {
            println!(
                "Routine state loop function instance({npc_instance_id}) loop_fn({}) error: {err:?}",
                loop_fn.value()
            );
        }

        std::mem::take(&mut self.npc_state_mut(npc_instance_id).routine.played_anis)
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
