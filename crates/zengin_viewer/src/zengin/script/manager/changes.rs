use std::collections::HashMap;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use bevy::math::Vec3;

use super::super::journal::{JournalChange, JournalManager, JournalTopic};
use super::super::state::{
    AlignToWpChange, AttackChange, ClearAiQueueChange, DialogLine, GotoChange, InventoryGain,
    ItemRuntime, LoadedState, NpcRuntime, OverlayChange, PendingChanges, RoutineExchange,
    ScreenMessage, SpawnItem, SpawnNpc,
};
use super::super::symbol_id::InstanceId;

impl super::ScriptManager {
    pub fn get_loaded_state(&self) -> &LoadedState {
        &self.store.data().loaded_state
    }

    pub fn npc_instances(&self) -> &HashMap<InstanceId, Arc<RwLock<NpcRuntime>>> {
        &self.store.data().loaded_state.npc_instances
    }

    /// Mutable runtime state of an npc, records the instance as changed.
    pub fn npc_state_mut(&mut self, index: InstanceId) -> RwLockWriteGuard<'_, NpcRuntime> {
        self.store.data_mut().npc_state_mut(index)
    }

    /// Runtime state of an npc, `None` when the instance has none.
    pub fn npc_state(&self, index: InstanceId) -> Option<RwLockReadGuard<'_, NpcRuntime>> {
        self.store.data().npc_state(index)
    }

    pub fn drain_npc_changes(&mut self) -> Vec<InstanceId> {
        std::mem::take(&mut self.pending_changes().npc_changed)
            .into_iter()
            .collect()
    }

    pub fn clear_npc_changes(&mut self) {
        self.pending_changes().npc_changed.clear();
    }

    pub fn drain_item_changes(&mut self) -> Vec<InstanceId> {
        std::mem::take(&mut self.pending_changes().item_changed)
            .into_iter()
            .collect()
    }

    pub fn clear_item_changes(&mut self) {
        self.pending_changes().item_changed.clear();
    }

    /// Mutable runtime state of an item, records the instance as changed.
    pub fn item_state_mut(&mut self, index: InstanceId) -> RwLockWriteGuard<'_, ItemRuntime> {
        self.store.data_mut().item_state_mut(index)
    }

    /// Runtime state of an item, `None` when the instance has none.
    #[allow(dead_code)] // no item consumers yet
    pub fn item_state(&self, index: InstanceId) -> Option<RwLockReadGuard<'_, ItemRuntime>> {
        self.store.data().item_state(index)
    }

    pub fn drain_routine_exchanges(&mut self) -> Vec<RoutineExchange> {
        std::mem::take(&mut self.pending_changes().routine_exchanges)
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

    pub fn drain_attack_changes(&mut self) -> Vec<AttackChange> {
        std::mem::take(&mut self.pending_changes().attack_changes)
    }

    pub fn drain_clear_ai_queue_changes(&mut self) -> Vec<ClearAiQueueChange> {
        std::mem::take(&mut self.pending_changes().clear_ai_queue_changes)
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

    /// Drops every pending change recorded so far. Used after a save restore to
    /// keep initialization/`init_*` side effects away from the ECS.
    pub fn clear_pending_changes(&mut self) {
        *self.pending_changes() = PendingChanges::default();
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

    /// Whether the player instance was set by a world spawn.
    pub fn has_player_instance(&self) -> bool {
        self.store.data().player_instance.is_some()
    }

    /// Instance id of the player (`PC_Hero`), `None` before the world spawn.
    pub fn player_instance(&self) -> Option<InstanceId> {
        self.store.data().player_instance
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
