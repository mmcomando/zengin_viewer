use std::sync::PoisonError;

use daedalus_typed::{TypedInstance, TypedRegistry};

use super::ImportCallState;
use crate::script::symbol_id::InstanceId;

impl ImportCallState {
    /// Equips `armor` on the npc, adding its `C_Item.protection` to the npc's
    /// `C_Npc.protection` and removing the protection of the armor it replaces.
    /// `None` unequips without adding anything.
    pub fn set_npc_armor(&mut self, npc: InstanceId, armor: Option<InstanceId>) {
        let old = self.npc_state(npc).and_then(|state| state.equipment.armor);
        if old == armor {
            return;
        }
        apply_armor_protection(&self.typed, npc, old, -1);
        self.npc_state_mut(npc).equipment.armor = armor;
        apply_armor_protection(&self.typed, npc, armor, 1);
    }
}

/// Adds `sign * armor.protection` to the npc's protection slots. `sign` is `-1`
/// when the armor is removed, the value is not clamped like the engine.
fn apply_armor_protection(
    typed: &TypedRegistry,
    npc: InstanceId,
    armor: Option<InstanceId>,
    sign: i32,
) {
    let Some(armor) = armor else {
        return;
    };
    let Some(protection) = typed.item(armor.value() as u32).map(|item| item.protection) else {
        return;
    };
    let Some(handle) = typed
        .instance(npc.value() as u32)
        .and_then(TypedInstance::npc)
        .cloned()
    else {
        return;
    };
    let mut data = handle.write().unwrap_or_else(PoisonError::into_inner);
    for (slot, value) in data.protection.iter_mut().zip(protection) {
        *slot += sign * value;
    }
}
