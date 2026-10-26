use std::collections::HashMap;

use bevy::math::Vec3;

use super::super::journal::{JournalChange, JournalManager, JournalTopic};
use super::super::state::{
    AlignToWpChange, DialogLine, EquipItemChange, GotoChange, HealthChange, InventoryGain,
    ItemInstanceState, LoadedState, NpcInstanceState, NpcLookAtChange, OverlayChange,
    PendingChanges, RoutineEntryChange, RoutineExchange, ScreenMessage, SpawnItem, SpawnNpc,
    WeaponModeChange,
};
use super::super::symbol_id::InstanceId;

impl super::ScriptManager {
    pub fn get_loaded_state(&self) -> &LoadedState {
        &self.store.data().loaded_state
    }

    pub fn npc_instances(&self) -> &HashMap<InstanceId, NpcInstanceState> {
        &self.store.data().loaded_state.npc_instances
    }

    pub fn npc_instances_mut(&mut self) -> &mut HashMap<InstanceId, NpcInstanceState> {
        &mut self.store.data_mut().loaded_state.npc_instances
    }

    pub fn npc_instances_mut_or_default(&mut self, index: InstanceId) -> &mut NpcInstanceState {
        self.npc_instances_mut().entry(index).or_default()
    }

    pub fn items_instances_mut(&mut self) -> &mut HashMap<InstanceId, ItemInstanceState> {
        &mut self.store.data_mut().loaded_state.items_instances
    }

    pub fn drain_routine_exchanges(&mut self) -> Vec<RoutineExchange> {
        std::mem::take(&mut self.pending_changes().routine_exchanges)
    }

    pub fn drain_routine_changes(&mut self) -> Vec<RoutineEntryChange> {
        std::mem::take(&mut self.pending_changes().routine_changes)
    }

    pub fn drain_look_at_changes(&mut self) -> Vec<NpcLookAtChange> {
        std::mem::take(&mut self.pending_changes().look_at_changes)
    }

    pub fn drain_weapon_mode_changes(&mut self) -> Vec<WeaponModeChange> {
        std::mem::take(&mut self.pending_changes().weapon_mode_changes)
    }

    pub fn drain_equip_item_changes(&mut self) -> Vec<EquipItemChange> {
        std::mem::take(&mut self.pending_changes().equip_item_changes)
    }

    #[allow(dead_code)] // used by the damage systems (next step)
    pub fn drain_health_changes(&mut self) -> Vec<HealthChange> {
        std::mem::take(&mut self.pending_changes().health_changes)
    }

    pub fn drain_align_to_wp_changes(&mut self) -> Vec<AlignToWpChange> {
        std::mem::take(&mut self.pending_changes().align_to_wp_changes)
    }

    pub fn drain_overlay_changes(&mut self) -> Vec<OverlayChange> {
        std::mem::take(&mut self.pending_changes().overlay_changes)
    }

    pub fn drain_goto_changes(&mut self) -> Vec<GotoChange> {
        std::mem::take(&mut self.pending_changes().goto_changes)
    }

    pub fn drain_spawn_items(&mut self) -> Vec<SpawnItem> {
        std::mem::take(&mut self.pending_changes().spawn_items)
    }

    pub fn drain_spawn_npcs(&mut self) -> Vec<SpawnNpc> {
        std::mem::take(&mut self.pending_changes().spawn_npcs)
    }

    pub fn loaded_state(&mut self) -> &LoadedState {
        let data = self.store.data_mut();
        &data.loaded_state
    }

    pub fn pending_changes(&mut self) -> &mut PendingChanges {
        &mut self.store.data_mut().pending
    }

    /// Lines only: sub dialog choices belong to their info and are cleared only
    /// by `Info_ClearChoices`.
    pub fn clear_dialog_lines(&mut self) {
        self.pending_changes().dialog_lines.clear();
    }

    pub fn drain_dialog_lines(&mut self) -> Vec<DialogLine> {
        std::mem::take(&mut self.pending_changes().dialog_lines)
    }

    pub fn drain_inventory_gains(&mut self) -> Vec<InventoryGain> {
        std::mem::take(&mut self.pending_changes().inventory_gains)
    }

    pub fn drain_journal_changes(&mut self) -> Vec<JournalChange> {
        std::mem::take(&mut self.pending_changes().journal_changes)
    }

    /// Snapshot of the journal topics in creation order.
    pub fn journal_topics(&self) -> Vec<JournalTopic> {
        self.store.data().journal.topics().to_vec()
    }

    pub fn journal_mut(&mut self) -> &mut JournalManager {
        &mut self.store.data_mut().journal
    }

    pub fn drain_screen_messages(&mut self) -> Vec<ScreenMessage> {
        std::mem::take(&mut self.pending_changes().screen_messages)
    }

    /// Records the instance id of the player (`PC_Hero`).
    pub fn set_player_instance(&mut self, player: InstanceId) {
        self.store.data_mut().player_instance = Some(player);
    }

    pub fn take_close_dialog(&mut self) -> bool {
        std::mem::take(&mut self.store.data_mut().close_dialog)
    }

    /// Fills the waypoint position table, keys must be lowercased.
    pub fn set_way_points(&mut self, way_points: HashMap<String, Vec3>) {
        self.store.data_mut().way_points = way_points;
    }

    /// Sets the day `Wld_GetDay` reports, counted from 0 on the first day.
    pub fn set_game_day(&mut self, day: u32) {
        self.store.data_mut().game_day = day;
    }
}
