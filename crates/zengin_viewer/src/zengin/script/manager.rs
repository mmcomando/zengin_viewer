use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use bevy::log::warn;
use daedalus_wasm::wasm::convert_daedalus_script;
use wasmtime::component::{Component, HasData, Linker};
use wasmtime::{Engine, Store};

use bevy::math::Vec3;

use super::bindings::MmWorld;
use super::bindings::exports::mmcomando::zengine_scripts::manage::{
    DialogInfo, ItemInfo, NpcInfo, SymbolInfo, SymbolKind,
};
use super::journal::JournalChange;
use super::state::{
    AlignToWpChange, DialogLine, DialogRoot, EquipItemChange, GotoChange, HealthChange,
    ImportCallState, InventoryGain, ItemInstanceState, LoadedState, NpcInstanceState,
    NpcLookAtChange, OverlayChange, PendingChanges, RoutineEntryChange, RoutineExchange, SpawnItem,
    SpawnNpc, SymbolTable, WeaponKind, WeaponMode, WeaponModeChange,
};
use super::string_value::{ShowString, StringCanon};
use super::symbol_id::{FunctionId, InstanceId, StringId, SymbolId};

pub struct ScriptManager {
    pub store: Store<ImportCallState>,
    pub instance: Arc<MmWorld>,
    /// Cache of resolved `_loop` function symbol ids of routine state
    /// functions, `None` when the state has no loop function.
    routine_loop_functions: HashMap<FunctionId, Option<FunctionId>>,
}

/// Display stats of an item instance for the inventory info panel, string
/// members resolved from the string table. The `get-item-info` record also
/// carries `value`/`weight`, currently not shown (the G2 script info lines
/// cover them).
#[derive(Debug, Default)]
pub struct ItemDisplayData {
    pub damage_total: i32,
    /// `C_Item.description` flavor text, `None` when unset.
    pub description: Option<String>,
    /// Equip requirements as (attribute display name, value) pairs.
    pub requirements: Vec<(&'static str, i32)>,
    /// Generic info-box lines as (text, value) pairs, unset lines skipped.
    pub info_lines: Vec<(String, i32)>,
}

/// Display names of the daedalus attribute indices items can require.
fn attribute_display_name(index: i32) -> Option<&'static str> {
    match index {
        // G2 script constants: ATR_STRENGTH = 4, ATR_DEXTERITY = 5
        4 => Some("Strength"),
        5 => Some("Dexterity"),
        _ => None,
    }
}

pub(crate) struct ScriptImports;

impl HasData for ScriptImports {
    type Data<'a> = &'a mut ImportCallState;
}

impl ScriptManager {
    pub fn get_loaded_state(&self) -> &LoadedState {
        &self.store.data().loaded_state
    }

    /// State of every loaded npc instance, keyed by instance id.
    pub fn npc_instances(&self) -> &HashMap<InstanceId, NpcInstanceState> {
        &self.store.data().loaded_state.npc_instances
    }

    /// Mutable state of every loaded npc instance, keyed by instance id.
    pub fn npc_instances_mut(&mut self) -> &mut HashMap<InstanceId, NpcInstanceState> {
        &mut self.store.data_mut().loaded_state.npc_instances
    }

    /// Mutable state of an npc instance, created with default values when missing.
    pub fn npc_instances_mut_or_default(&mut self, index: InstanceId) -> &mut NpcInstanceState {
        self.npc_instances_mut().entry(index).or_default()
    }

    // /// State of every loaded item instance, keyed by instance id.
    // pub fn items_instances(&self) -> &HashMap<InstanceId, ItemInstanceState> {
    //     &self.store.data().loaded_state.items_instances
    // }

    /// Mutable state of every loaded item instance, keyed by instance id.
    pub fn items_instances_mut(&mut self) -> &mut HashMap<InstanceId, ItemInstanceState> {
        &mut self.store.data_mut().loaded_state.items_instances
    }

    /// Drains the `Npc_ExchangeRoutine` requests the script produced, for the
    /// ECS bridge.
    pub fn drain_routine_exchanges(&mut self) -> Vec<RoutineExchange> {
        std::mem::take(&mut self.pending_changes().routine_exchanges)
    }

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

    /// Drains the routine entry changes the script produced, for the ECS bridge.
    pub fn drain_routine_changes(&mut self) -> Vec<RoutineEntryChange> {
        std::mem::take(&mut self.pending_changes().routine_changes)
    }

    /// Drains the look-at target changes the script produced, for the ECS bridge.
    pub fn drain_look_at_changes(&mut self) -> Vec<NpcLookAtChange> {
        std::mem::take(&mut self.pending_changes().look_at_changes)
    }

    /// Drains the weapon mode changes the script produced, for the ECS bridge.
    pub fn drain_weapon_mode_changes(&mut self) -> Vec<WeaponModeChange> {
        std::mem::take(&mut self.pending_changes().weapon_mode_changes)
    }

    /// Drains the equipped item changes the script produced, for the ECS bridge.
    pub fn drain_equip_item_changes(&mut self) -> Vec<EquipItemChange> {
        std::mem::take(&mut self.pending_changes().equip_item_changes)
    }

    /// Drains the hitpoints changes the host produced, for the ECS bridge.
    #[allow(dead_code)] // used by the damage systems (next step)
    pub fn drain_health_changes(&mut self) -> Vec<HealthChange> {
        std::mem::take(&mut self.pending_changes().health_changes)
    }

    /// Drains the `AI_AlignToWP` requests the script produced, for the ECS bridge.
    pub fn drain_align_to_wp_changes(&mut self) -> Vec<AlignToWpChange> {
        std::mem::take(&mut self.pending_changes().align_to_wp_changes)
    }

    /// Drains the overlay change requests the script produced, for the ECS bridge.
    pub fn drain_overlay_changes(&mut self) -> Vec<OverlayChange> {
        std::mem::take(&mut self.pending_changes().overlay_changes)
    }

    /// Drains the `AI_GoToWP` requests the script produced, for the ECS bridge.
    pub fn drain_goto_changes(&mut self) -> Vec<GotoChange> {
        std::mem::take(&mut self.pending_changes().goto_changes)
    }

    /// Drains the item spawn requests queued at world load, for the ECS bridge.
    pub fn drain_spawn_items(&mut self) -> Vec<SpawnItem> {
        std::mem::take(&mut self.pending_changes().spawn_items)
    }

    /// Drains the npc spawn requests queued by `wld_insertnpc`, for the ECS
    /// bridge.
    pub fn drain_spawn_npcs(&mut self) -> Vec<SpawnNpc> {
        std::mem::take(&mut self.pending_changes().spawn_npcs)
    }

    /// Loaded state and the pending spawn item buffer, for the world load which
    /// reads the instance data and queues the item spawns.
    pub fn loaded_state(&mut self) -> &LoadedState {
        let data = self.store.data_mut();
        &data.loaded_state
    }

    /// Queued script changes and requests, drained to the ECS by the script
    /// bridge.
    pub fn pending_changes(&mut self) -> &mut PendingChanges {
        &mut self.store.data_mut().pending
    }

    pub fn clear_dialog_outputs(&mut self) {
        self.pending_changes().dialog_lines.clear();
        self.pending_changes().sub_dialog_choices.clear();
    }

    /// Drains the dialog lines output by the script, for the ECS bridge.
    pub fn drain_dialog_lines(&mut self) -> Vec<DialogLine> {
        std::mem::take(&mut self.pending_changes().dialog_lines)
    }

    /// Drains the sub dialog choices added by the script, for the ECS bridge.
    pub fn drain_sub_dialog_choices(&mut self) -> Vec<DialogRoot> {
        std::mem::take(&mut self.pending_changes().sub_dialog_choices)
    }

    /// Drains the player inventory gains the script produced, for the ECS
    /// bridge game log.
    pub fn drain_inventory_gains(&mut self) -> Vec<InventoryGain> {
        std::mem::take(&mut self.pending_changes().inventory_gains)
    }

    /// Drains the journal changes the script produced, for the ECS bridge game
    /// log.
    pub fn drain_journal_changes(&mut self) -> Vec<JournalChange> {
        std::mem::take(&mut self.pending_changes().journal_changes)
    }

    /// Records the instance id of the player (`PC_Hero`), used by
    /// `Npc_IsPlayer` and the inventory-gain log.
    pub fn set_player_instance(&mut self, player: InstanceId) {
        self.store.data_mut().player_instance = Some(player);
    }

    /// Takes the dialog close request, leaving it cleared
    pub fn take_close_dialog(&mut self) -> bool {
        std::mem::take(&mut self.store.data_mut().close_dialog)
    }

    /// Registers the info instance as known by the npc, used when a dialog
    /// root is actually played
    pub fn mark_info_known(
        &mut self,
        npc_instance_id: InstanceId,
        info_instance_id: Option<InstanceId>,
    ) {
        let Some(info_instance_id) = info_instance_id else {
            return;
        };
        self.npc_instances_mut_or_default(npc_instance_id)
            .known_infos
            .insert(info_instance_id);
    }

    pub fn from_file_cached(script_path: &Path, script_wasm_cache_path: &Path) -> Self {
        // if script_wasm_cache_path.is_file() {
        //     let wasm_bytes = std::fs::read(script_wasm_cache_path).unwrap();
        //     return Self::new(&wasm_bytes);
        // }
        let wasm_bytes = convert_daedalus_script(script_path);
        std::fs::write(script_wasm_cache_path, &wasm_bytes).unwrap();
        return Self::new(&wasm_bytes);
    }

    pub fn new(wasm_bytes: &[u8]) -> Self {
        let engine = Engine::default();
        let component = Component::from_binary(&engine, wasm_bytes).unwrap();
        let mut linker = Linker::new(&engine);
        MmWorld::add_to_linker::<ImportCallState, ScriptImports>(
            &mut linker,
            |state: &mut ImportCallState| -> &mut ImportCallState { state },
        )
        .unwrap();

        let my_state = ImportCallState::default();
        let mut store = Store::new(&engine, my_state);
        let instance = MmWorld::instantiate(&mut store, &component, &linker).unwrap();
        let instance2 = MmWorld::instantiate(&mut store, &component, &linker).unwrap();
        let instance_arc = Arc::new(instance);
        store.data_mut().world = Some(Arc::new(instance2));

        Self {
            store,
            instance: instance_arc,
            routine_loop_functions: HashMap::new(),
        }
    }

    /// Moves all items of `from`'s inventory into `to`'s inventory, returning the
    /// moved `(item, count)` pairs.
    pub fn transfer_inventory(
        &mut self,
        from: InstanceId,
        to: InstanceId,
    ) -> Vec<(InstanceId, i32)> {
        let taken = self
            .npc_instances_mut()
            .get_mut(&from)
            .map(|npc| npc.inventory.take_all());
        let Some(taken) = taken else {
            return Vec::new();
        };
        let moved = taken.items();
        self.npc_instances_mut_or_default(to).inventory.merge(taken);
        moved
    }

    /// Adds `count` of `item` to an npc's inventory.
    pub fn add_inventory(&mut self, npc: InstanceId, item: InstanceId, count: i32) {
        self.npc_instances_mut_or_default(npc)
            .inventory
            .create(item, count);
    }

    /// Removes up to `count` of `item` from an npc's inventory, returns how
    /// many were actually removed.
    pub fn remove_inventory(&mut self, npc: InstanceId, item: InstanceId, count: i32) -> i32 {
        self.npc_instances_mut_or_default(npc)
            .inventory
            .remove(item, count)
    }

    /// Equips an item as the npc's melee or ranged weapon depending on its
    /// instance symbol name (`EquipItem` semantics), leaving the change queued
    /// for the ECS bridge. Equipping the same slot again replaces the weapon.
    pub fn equip_item(&mut self, npc: InstanceId, item: InstanceId) {
        let kind = self
            .symbol_table()
            .symbol_name(item.value())
            .as_deref()
            .and_then(WeaponKind::from_symbol_name);
        let Some(kind) = kind else {
            warn!(
                "Cannot equip item({}) on npc({}): not a known weapon kind",
                item.value(),
                npc.value()
            );
            return;
        };
        let instance = self.npc_instances_mut_or_default(npc);
        match kind {
            WeaponKind::Melee => instance.equipped_melee = Some(item),
            WeaponKind::Bow | WeaponKind::Crossbow => instance.equipped_ranged = Some(item),
        }
        self.pending_changes()
            .equip_item_changes
            .push(EquipItemChange { instance_id: npc });
    }

    /// Draws the npc's weapon into the hand (`Npc_SetToFightMode` semantics),
    /// leaving the change queued for the ECS bridge. `ranged` selects the
    /// ranged weapon, otherwise the melee one is drawn.
    pub fn draw_weapon(&mut self, npc: InstanceId, ranged: bool) {
        let mode = if ranged {
            WeaponMode::Ranged
        } else {
            WeaponMode::Melee
        };
        let equipped = self.npc_instances().get(&npc).map(|npc| {
            if ranged {
                npc.equipped_ranged.is_some()
            } else {
                npc.equipped_melee.is_some()
            }
        });
        if equipped != Some(true) {
            warn!(
                "Cannot draw {} weapon of npc({}): nothing equipped",
                if ranged { "ranged" } else { "melee" },
                npc.value()
            );
            return;
        }
        self.npc_instances_mut_or_default(npc).weapon_mode = Some(mode);
        self.pending_changes()
            .weapon_mode_changes
            .push(WeaponModeChange { instance_id: npc });
    }

    /// Sheathes the drawn weapon back to its stowed position
    /// (`Npc_SetToFistMode`/`AI_RemoveWeapon` semantics), leaving the change
    /// queued for the ECS bridge. The weapon stays equipped.
    #[allow(dead_code)] // used by the draw/sheathe weapon inputs
    pub fn sheathe_weapon(&mut self, npc: InstanceId) {
        self.npc_instances_mut_or_default(npc).weapon_mode = Some(WeaponMode::Fists);
        self.pending_changes()
            .weapon_mode_changes
            .push(WeaponModeChange { instance_id: npc });
    }

    /// Unequips the npc's weapon of the given kind, sheathing it when it was
    /// drawn. Leaves the changes queued for the ECS bridge.
    pub fn unequip_weapon_kind(&mut self, npc: InstanceId, kind: WeaponKind) {
        let instance = self.npc_instances_mut_or_default(npc);
        match kind {
            WeaponKind::Melee => instance.equipped_melee = None,
            WeaponKind::Bow | WeaponKind::Crossbow => instance.equipped_ranged = None,
        }
        let drawn_kind_matches = match instance.weapon_mode {
            Some(WeaponMode::Melee) => kind == WeaponKind::Melee,
            Some(WeaponMode::Ranged) => kind.is_ranged(),
            _ => false,
        };
        if drawn_kind_matches {
            instance.weapon_mode = Some(WeaponMode::Fists);
            self.pending_changes()
                .weapon_mode_changes
                .push(WeaponModeChange { instance_id: npc });
        }
        self.pending_changes()
            .equip_item_changes
            .push(EquipItemChange { instance_id: npc });
    }

    /// Unequips all of the npc's weapons and sheathes them (`AI_RemoveWeapon`
    /// and clearing `EquipItem` semantics), leaving the changes queued for the
    /// ECS bridge.
    #[allow(dead_code)] // semantics kept for the AI_UnEquipWeapons host import
    pub fn unequip_item(&mut self, npc: InstanceId) {
        let instance = self.npc_instances_mut_or_default(npc);
        instance.equipped_melee = None;
        instance.equipped_ranged = None;
        instance.weapon_mode = Some(WeaponMode::Fists);
        self.pending_changes()
            .equip_item_changes
            .push(EquipItemChange { instance_id: npc });
        self.pending_changes()
            .weapon_mode_changes
            .push(WeaponModeChange { instance_id: npc });
    }

    /// Contents of an npc's inventory as `(item, count)` pairs, empty when the
    /// npc has no inventory.
    pub fn inventory(&self, npc: InstanceId) -> Vec<(InstanceId, i32)> {
        self.npc_instances()
            .get(&npc)
            .map(|npc| npc.inventory.items())
            .unwrap_or_default()
    }

    /// Fills the waypoint position table used by `Npc_GetDistToWP`, keys must be
    /// lowercased.
    pub fn set_way_points(&mut self, way_points: HashMap<String, Vec3>) {
        self.store.data_mut().way_points = way_points;
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

    pub fn call_get_name(&mut self, symbol_id: SymbolId) -> String {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_get_name(&mut self.store, symbol_id.value())
            .unwrap()
    }

    /// Fetches the content of the string table entry `raw`, `None` only when
    /// the id is out of range. Empty strings are valid values, callers filter
    /// them out when emptiness means "none". Only works for static string
    /// literal ids, resolved by a table loaded at startup instead of calling
    /// into the guest.
    fn get_string_non_0(&self, raw: i32) -> Option<String> {
        self.store.data().symbol_table.string_content(raw)
    }

    /// Fetches a static string literal with its original case, `None` when it
    /// is empty
    pub fn get_show_string(&mut self, id: StringId) -> Option<ShowString> {
        let string = self.get_string_non_0(id.value())?;
        Some(ShowString::new(string))
    }

    /// Fetches a static string literal as a canonical (lowercased) lookup key,
    /// for names matched case-insensitively (waypoints, spots), `None` when it
    /// is empty
    pub fn get_canon_string(&mut self, id: StringId) -> Option<StringCanon> {
        let string = self.get_string_non_0(id.value())?;
        Some(StringCanon::new(string))
    }

    /// Resolves the name of a raw symbol id, `None` when it is not a valid id.
    pub fn get_name_by_id(&mut self, raw: i32) -> Option<String> {
        let symbol = SymbolId::new(raw)?;
        Some(self.call_get_name(symbol))
    }

    /// Content of a daedalus string id, `None` when unset or empty.
    fn string_by_id(&self, raw: Option<i32>) -> Option<String> {
        self.get_string_non_0(raw?)
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

    /// Raw npc-info record of an npc instance, read from the instance data.
    fn npc_info(&mut self, npc: InstanceId) -> NpcInfo {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_get_npc_info(&mut self.store, npc.symbol().value())
            .unwrap()
    }

    /// Raw item-info record of an item instance, read from the instance data.
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

    /// Total damage of an item instance (`C_Item.damagetotal`), `None` when
    /// the item instance has no data.
    pub fn item_damage_total(&mut self, item: InstanceId) -> Option<i32> {
        Some(self.item_info(item).damage_total)
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
            description: self.string_by_id(info.description),
            requirements,
            info_lines,
        }
    }

    pub fn get_symbol_id(&mut self, symbol_name: &str) -> SymbolId {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        let symbol_id = guest
            .call_get_symbol_id(&mut self.store, symbol_name)
            .unwrap()
            .expect("Symbol should exist");
        SymbolId::new(symbol_id).expect("Symbol id out of range")
    }

    /// Resolves a function symbol by name, verifying that it is a function.
    /// Panics when the symbol does not exist or is not a function.
    pub fn get_function_id(&mut self, symbol_name: &str) -> FunctionId {
        let symbol_id = self.get_symbol_id(symbol_name);
        self.symbol_table()
            .function(symbol_id.value())
            .expect("Symbol is not a function")
    }

    pub fn symbol_table(&self) -> &SymbolTable {
        &self.store.data().symbol_table
    }

    /// Resolves an instance symbol by name, `None` when the symbol does not
    /// exist or is not an instance.
    pub fn symbol_instance(&mut self, symbol_name: &str) -> Option<InstanceId> {
        let symbol = self.try_get_symbol_id(symbol_name)?;
        self.symbol_table().instance(symbol.value())
    }

    /// Loads the kind of every symbol into the symbol table used to verify
    /// typed symbol ids. Must be called before script functions are executed,
    /// host calls made during initialization already verify ids.
    pub fn load_symbol_table(&mut self) {
        let symbols_count = self.get_all_symbols_num();
        let mut kinds = HashMap::with_capacity(symbols_count.max(0) as usize);
        for index in 0..symbols_count {
            let Some(symbol_id) = SymbolId::new(index) else {
                continue;
            };
            kinds.insert(index, self.get_symbol_info(symbol_id).kind);
        }

        // Load all string pair contents once at startup (symbol names for ids
        // below `symbols_count`, static string literals above), so import
        // handlers and host code resolve strings by table lookup instead of
        // calling back into the guest while a script function is running
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        let strings_count = guest.call_get_all_strings_num(&mut self.store).unwrap();
        let mut strings = Vec::with_capacity(strings_count.max(0) as usize);
        for raw in 0..strings_count {
            let string = guest.call_get_name(&mut self.store, raw).unwrap();
            strings.push(string);
        }

        self.store.data_mut().symbol_table = SymbolTable::new(kinds, symbols_count, strings);
    }

    /// Resolves a symbol id by name, `None` when the symbol does not exist.
    /// The guest lookup does not trap for missing symbols, a trap would make
    /// the component instance unusable for all further calls.
    pub fn try_get_symbol_id(&mut self, symbol_name: &str) -> Option<SymbolId> {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_get_symbol_id(&mut self.store, symbol_name)
            .unwrap()
            .and_then(SymbolId::new)
    }

    fn try_call_function(&mut self, symbol_id: FunctionId) -> wasmtime::Result<()> {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest.call_call_function(&mut self.store, symbol_id.value())
    }

    /// Calls a function ignoring its result. Dialog functions can be declared
    /// `func int` while the dialog code expects void, so the guest picks the
    /// indirect call type based on the symbol's return flag.
    fn try_call_function_discard(&mut self, symbol_id: FunctionId) -> wasmtime::Result<()> {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest.call_call_function_discard(&mut self.store, symbol_id.value())
    }

    /// Calls a function returning a value (`ZS_*_loop` functions do). The guest
    /// function table is typed, so such functions must use the bool call path
    /// or the call would trap on the signature mismatch.
    fn try_call_function_bool(&mut self, symbol_id: FunctionId) -> wasmtime::Result<bool> {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest.call_call_function_bool(&mut self.store, symbol_id.value())
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

    pub fn call_function(&mut self, symbol_id: FunctionId) {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_call_function(&mut self.store, symbol_id.value())
            .unwrap();
    }

    pub fn call_function_bool(&mut self, symbol_id: FunctionId) -> bool {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_call_function_bool(&mut self.store, symbol_id.value())
            .unwrap()
    }

    /// Evaluates the condition function of every root with the npc instance
    /// set, dropping roots whose condition returns false
    pub fn filter_dialog_roots(
        &mut self,
        npc_player: InstanceId,
        npc_talk_to: InstanceId,
        roots: &mut Vec<DialogRoot>,
    ) {
        roots.retain(|root| {
            let Some(condition_fn) = root.condition_function_id else {
                return true;
            };
            if !root.permanent
                && let Some(info_instance_id) = root.info_instance_id
                && let Some(infos) = self
                    .npc_instances()
                    .get(&npc_player)
                    .map(|npc| &npc.known_infos)
                && infos.contains(&info_instance_id)
            {
                return false;
            }

            self.set_instance(npc_talk_to);
            self.set_self(npc_talk_to);
            self.set_other(npc_player);
            self.call_function_bool(condition_fn)
        });
    }

    pub fn initialize_instances(&mut self) {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_initialize_all_instances(&mut self.store)
            .unwrap();
    }

    pub fn get_symbol_info(&mut self, symbol_id: SymbolId) -> SymbolInfo {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_get_symbol_info(&mut self.store, symbol_id.value())
            .unwrap()
    }

    pub fn get_dialog_info(&mut self, symbol_id: SymbolId) -> DialogInfo {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_get_dialog_info(&mut self.store, symbol_id.value())
            .unwrap()
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

    pub fn get_all_symbols_num(&mut self) -> i32 {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest.call_get_all_symbols_num(&mut self.store).unwrap()
    }

    fn has_ancestor(&mut self, symbol_info: &SymbolInfo, ancestor_id: SymbolId) -> bool {
        let Some(parent) = symbol_info.parent else {
            return false;
        };
        if parent == ancestor_id.value() {
            return true;
        }
        let symbol_info = self.get_symbol_info(SymbolId::new(parent).unwrap());

        return self.has_ancestor(&symbol_info, ancestor_id);
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

    pub fn load_dialogs_information(&mut self) {
        let symbols_num = self.get_all_symbols_num();

        let info_class_id = self.get_symbol_id("c_info");

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
            if !self.has_ancestor(&symbol_info, info_class_id) {
                continue;
            }

            let dialog_info = self.get_dialog_info(index.symbol());
            let Some(sequence_function_id) = self.symbol_table().function(dialog_info.information)
            else {
                continue;
            };
            let description = self
                .symbol_table()
                .string(dialog_info.description)
                .and_then(|id| self.get_show_string(id))
                .map(super::string_value::ShowString::into_string)
                .unwrap_or_default();

            let Some(npc_id) = self.symbol_table().instance(dialog_info.npc) else {
                continue;
            };
            let condition_function_id = self.symbol_table().function(dialog_info.condition);
            let npc = self.npc_instances_mut_or_default(npc_id);
            let roots = npc.dialog_roots.get_or_insert_with(Vec::new);
            roots.push(DialogRoot {
                info_instance_id: Some(index),
                sequence_function_id,
                condition_function_id,
                permanent: dialog_info.permanent,
                description,
                important: dialog_info.important,
                order: dialog_info.order,
            });

            roots.sort_by_key(|el| el.order);
        }
    }

    pub fn call_dialog_sequence_function(&mut self, sequence_function_id: FunctionId) {
        self.store.data_mut().close_dialog = false;
        match self.try_call_function_discard(sequence_function_id) {
            Ok(()) => {}
            Err(err) => {
                println!("call_dialog_sequence_function({sequence_function_id:?}) error: {err:?}");
            }
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
                .map(super::string_value::ShowString::into_string)
            else {
                println!(
                    "Not found body model string for npc({index}), body_model({})",
                    body_model_id.map(super::symbol_id::StringId::value).unwrap_or_default()
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
            entry.hierarchy = hierarchy.map(super::string_value::ShowString::into_string);
        }
    }
}
