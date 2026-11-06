use bevy::log::warn;

use super::super::state::{EquipItemChange, WeaponKind, WeaponMode, WeaponModeChange};
use super::super::symbol_id::InstanceId;

impl super::ScriptManager {
    /// Moves all items of `from`'s inventory into `to`'s inventory, returning the
    /// moved `(item, count)` pairs.
    pub fn transfer_inventory(
        &mut self,
        from: InstanceId,
        to: InstanceId,
    ) -> Vec<(InstanceId, i32)> {
        let taken = self.npc_state_mut(from).inventory.take_all();
        let moved = taken.items();
        if moved.is_empty() {
            return Vec::new();
        }
        self.npc_state_mut(to).inventory.merge(taken);
        moved
    }

    pub fn add_inventory(&mut self, npc: InstanceId, item: InstanceId, count: i32) {
        self.npc_state_mut(npc).inventory.create(item, count);
    }

    /// Removes up to `count` of `item` from an npc's inventory, returns how
    /// many were actually removed.
    pub fn remove_inventory(&mut self, npc: InstanceId, item: InstanceId, count: i32) -> i32 {
        self.npc_state_mut(npc).inventory.remove(item, count)
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
        {
            let mut instance = self.npc_state_mut(npc);
            match kind {
                WeaponKind::Melee => instance.equipment.melee = Some(item),
                WeaponKind::Bow | WeaponKind::Crossbow => instance.equipment.ranged = Some(item),
            }
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
        let equipped = self.npc_state(npc).map(|npc| {
            if ranged {
                npc.equipment.ranged.is_some()
            } else {
                npc.equipment.melee.is_some()
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
        self.npc_state_mut(npc).equipment.weapon_mode = Some(mode);
        self.pending_changes()
            .weapon_mode_changes
            .push(WeaponModeChange { instance_id: npc });
    }

    /// Sheathes the drawn weapon back to its stowed position
    /// (`Npc_SetToFistMode`/`AI_RemoveWeapon` semantics), leaving the change
    /// queued for the ECS bridge. The weapon stays equipped.
    #[allow(dead_code)] // used by the draw/sheathe weapon inputs
    pub fn sheathe_weapon(&mut self, npc: InstanceId) {
        self.npc_state_mut(npc).equipment.weapon_mode = Some(WeaponMode::Fists);
        self.pending_changes()
            .weapon_mode_changes
            .push(WeaponModeChange { instance_id: npc });
    }

    /// Unequips the npc's weapon of the given kind, sheathing it when it was
    /// drawn. Leaves the changes queued for the ECS bridge.
    pub fn unequip_weapon_kind(&mut self, npc: InstanceId, kind: WeaponKind) {
        let drawn_kind_matches = {
            let mut instance = self.npc_state_mut(npc);
            match kind {
                WeaponKind::Melee => instance.equipment.melee = None,
                WeaponKind::Bow | WeaponKind::Crossbow => instance.equipment.ranged = None,
            }
            match instance.equipment.weapon_mode {
                Some(WeaponMode::Melee) => kind == WeaponKind::Melee,
                Some(WeaponMode::Ranged) => kind.is_ranged(),
                _ => false,
            }
        };
        if drawn_kind_matches {
            self.npc_state_mut(npc).equipment.weapon_mode = Some(WeaponMode::Fists);
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
        {
            let mut instance = self.npc_state_mut(npc);
            instance.equipment.melee = None;
            instance.equipment.ranged = None;
            instance.equipment.weapon_mode = Some(WeaponMode::Fists);
        }
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
        self.npc_state(npc)
            .map(|npc| npc.inventory.items())
            .unwrap_or_default()
    }
}
