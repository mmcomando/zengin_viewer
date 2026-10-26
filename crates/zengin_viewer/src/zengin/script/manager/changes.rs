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

    /// Lines only: sub dialog choices belong to their info and are cleared only
    /// by `Info_ClearChoices`.
    pub fn clear_dialog_lines(&mut self) {
        self.pending_changes().dialog_lines.clear();
    }

    /// Drains the dialog lines output by the script, for the ECS bridge.
    pub fn drain_dialog_lines(&mut self) -> Vec<DialogLine> {
        std::mem::take(&mut self.pending_changes().dialog_lines)
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

    /// Snapshot of the journal topics in creation order, for the journal UI.
    pub fn journal_topics(&self) -> Vec<JournalTopic> {
        self.store.data().journal.topics().to_vec()
    }

    /// Mutable journal, used by the debug helpers to seed test topics.
    pub fn journal_mut(&mut self) -> &mut JournalManager {
        &mut self.store.data_mut().journal
    }

    /// Drains the on-screen messages the script requested, for the ECS bridge.
    pub fn drain_screen_messages(&mut self) -> Vec<ScreenMessage> {
        std::mem::take(&mut self.pending_changes().screen_messages)
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

    /// Fills the waypoint position table used by `Npc_GetDistToWP`, keys must be
    /// lowercased.
    pub fn set_way_points(&mut self, way_points: HashMap<String, Vec3>) {
        self.store.data_mut().way_points = way_points;
    }
}
