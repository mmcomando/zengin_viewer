use bevy::log::warn;

use super::super::bindings::exports::mmcomando::zengine_scripts::manage::{
    ItemInfo, NpcInfo, SymbolKind,
};
use super::super::state::HealthChange;
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
        self.npc_instances_mut_or_default(npc)
            .routine_entries
            .clear();
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
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_set_self(&mut self.store, instance_index.value())
            .unwrap();
    }

    /// Binds the given instance as `other` for subsequent script calls
    pub fn set_other(&mut self, instance_index: InstanceId) {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_set_other(&mut self.store, instance_index.value())
            .unwrap();
    }

    pub fn call_npc_routine(&mut self, instance_index: InstanceId) {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        let result = guest.call_call_npc_routine(&mut self.store, instance_index.value());
        match result {
            Ok(()) => {}
            Err(err) => {
                println!("call_npc_routine({instance_index}) error: {:?}", err);
            }
        }
    }

    /// Display name of an npc from its `C_NPC` `name` member, `None` when the
    /// instance has no data or the name string is unset.
    pub fn npc_display_name(&mut self, npc: InstanceId) -> Option<String> {
        let name = self.npc_info(npc).name;
        self.string_by_id(name)
    }

    /// Display name of an item from its `C_ITEM` `name` member, `None` when
    /// the instance has no data or the name string is unset.
    pub fn item_display_name(&mut self, item: InstanceId) -> Option<String> {
        let name = self.item_info(item).name;
        self.string_by_id(name)
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

    fn npc_info(&mut self, npc: InstanceId) -> NpcInfo {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_get_npc_info(&mut self.store, npc.symbol().value())
            .unwrap()
    }

    fn item_info(&mut self, item: InstanceId) -> ItemInfo {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_get_item_info(&mut self.store, item.symbol().value())
            .unwrap()
    }

    /// Hitpoints and hitpoints max of an npc, read from the npc instance data
    /// (`ZkNpcInstance_getAttribute` semantics, attribute `ATR_HITPOINTS` and
    /// `ATR_HITPOINTS_MAX`). Seeds the instance state on first access, `None`
    /// when the id is not an npc instance.
    pub fn hitpoints(&mut self, npc: InstanceId) -> Option<(i32, i32)> {
        let seeded = self
            .npc_instances()
            .get(&npc)
            .is_some_and(|npc| npc.hitpoints.is_some());
        if !seeded {
            let info = self.npc_info(npc);
            let instance = self.npc_instances_mut_or_default(npc);
            instance.hitpoints = Some(info.hitpoints);
            instance.hitpoints_max = Some(info.hitpoints_max);
            self.pending_changes()
                .health_changes
                .push(HealthChange { instance_id: npc });
        }
        let instance = self.npc_instances().get(&npc)?;
        Some((instance.hitpoints?, instance.hitpoints_max?))
    }

    /// Changes the hitpoints of an npc by `delta` (damage is negative),
    /// clamped to `0..=hitpoints_max`. Leaves the change queued for the ECS
    /// bridge. Returns the new hitpoints, `None` when not seeded yet.
    #[allow(dead_code)] // used by the damage systems (next step)
    pub fn change_hitpoints(&mut self, npc: InstanceId, delta: i32) -> Option<i32> {
        self.hitpoints(npc)?;
        let instance = self.npc_instances_mut_or_default(npc);
        let max = instance.hitpoints_max.unwrap_or_default();
        let current = instance.hitpoints.unwrap_or_default();
        let changed = (current + delta).clamp(0, max);
        instance.hitpoints = Some(changed);
        self.pending_changes()
            .health_changes
            .push(HealthChange { instance_id: npc });
        Some(changed)
    }

    /// Total damage of an item instance (`C_Item.damagetotal`).
    pub fn item_damage_total(&mut self, item: InstanceId) -> i32 {
        self.item_info(item).damage_total
    }

    /// Display stats of an item instance for the inventory info panel.
    pub fn item_display_data(&mut self, item: InstanceId) -> ItemDisplayData {
        let info = self.item_info(item);
        let requirements = info
            .cond_attribute
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
                // Text string id 0 means the line is unset; string id 0 itself
                // resolves to an unrelated table entry, so skip it explicitly
                let text = self.string_by_id((text != 0).then_some(text))?;
                Some((text, count))
            })
            .collect();
        ItemDisplayData {
            damage_total: info.damage_total,
            value: info.value,
            description: self.string_by_id(info.description),
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
        self.npc_instances_mut_or_default(npc_instance_id)
            .routine_played_anis
            .clear();
        // Track the active state so `Npc_IsInState`/`Npc_WasInState` answer
        // while the state function runs
        {
            let npc = self.npc_instances_mut_or_default(npc_instance_id);
            if let Some(previous) = npc.current_routine_state.replace(state_fn)
                && previous != state_fn
            {
                npc.previous_routine_state = Some(previous);
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

        self.npc_instances_mut()
            .get_mut(&npc_instance_id)
            .map(|npc| std::mem::take(&mut npc.routine_played_anis))
            .unwrap_or_default()
    }

    pub fn get_item_visual(&mut self, symbol_id: SymbolId) -> Option<String> {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_get_item_visual(&mut self.store, symbol_id.value())
            .unwrap()
    }
    pub fn get_item_visual_change(&mut self, symbol_id: SymbolId) -> Option<String> {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_get_item_visual_change(&mut self.store, symbol_id.value())
            .unwrap()
    }

    pub fn call_npc_routines(&mut self) {
        let npc_indices: Vec<_> = self
            .npc_instances()
            .iter()
            .filter(|(_, npc)| npc.visual_initialized)
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
            let entry = self.items_instances_mut().entry(index).or_default();
            entry.name = symbol_name;
            entry.visual = visual;
            entry.visual_change = visual_change;
        }
    }

    pub fn load_npc_instances(&mut self) {
        let indices: Vec<InstanceId> = self
            .npc_instances()
            .iter()
            .filter(|(_, npc)| npc.visual_initialized)
            .map(|(index, _)| *index)
            .collect();

        for index in indices {
            // Copy the raw visual body ids out before resolving them through
            // the guest (which needs `&mut self`).
            let Some(raw) = self.npc_instances().get(&index).map(|npc| {
                (
                    npc.body_model_id,
                    npc.head_model_id,
                    npc.armor_model_id,
                    npc.hierarchy_id,
                    npc.face_texture_index,
                    npc.body_texture_index,
                )
            }) else {
                continue;
            };
            let (
                body_model_id,
                head_model_id,
                armor_model_id,
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
            // The armor id is an item instance symbol, its name must match
            // the item instance names used by the `armor_visual` lookup.
            // Empty strings are valid script values meaning "none" (e.g.
            // animals pass "" as the head model), filter them out here.
            let armor_model = armor_model_id
                .and_then(|id| self.get_string_non_0(id.value()))
                .filter(|armor_model| !armor_model.is_empty());
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

            let entry = self.npc_instances_mut_or_default(index);
            entry.body_model = body_model;
            entry.body_texture = body_texture;
            entry.head_model = head_model;
            entry.face_texture = face_texture;
            entry.armor_model = armor_model;
            entry.hierarchy = hierarchy.map(ShowString::into_string);
        }
    }
}
