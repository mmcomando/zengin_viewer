//! Inventory of one npc: item instance id to count, with the modification
//! operations the script host imports and the ECS-side managers need.

use std::collections::HashMap;

use super::symbol_id::InstanceId;

/// Items owned by one npc, keyed by item instance id.
#[derive(Debug, Default)]
pub struct Inventory {
    items: HashMap<InstanceId, i32>,
}

impl Inventory {
    /// Count of `item` in the inventory, `0` when absent.
    pub fn count(&self, item: InstanceId) -> i32 {
        self.items.get(&item).copied().unwrap_or(0)
    }

    /// Contents as `(item, count)` pairs.
    pub fn items(&self) -> Vec<(InstanceId, i32)> {
        self.items
            .iter()
            .map(|(item, count)| (*item, *count))
            .collect()
    }

    /// Adds `count` of `item`.
    pub fn create(&mut self, item: InstanceId, count: i32) {
        if count <= 0 {
            return;
        }
        *self.items.entry(item).or_insert(0) += count;
    }

    /// Removes the whole stack of `item`.
    pub fn take(&mut self, item: InstanceId) -> i32 {
        self.items.remove(&item).unwrap_or(0)
    }

    /// Removes up to `amount` of `item`.
    pub fn remove(&mut self, item: InstanceId, amount: i32) -> i32 {
        if amount < 1 {
            return 0;
        }
        let Some(count) = self.items.get_mut(&item) else {
            return 0;
        };
        let removed = (*count).min(amount);
        *count -= removed;
        if *count <= 0 {
            self.items.remove(&item);
        }
        removed
    }

    /// Empties the inventory, returning the removed contents.
    pub fn take_all(&mut self) -> Inventory {
        Inventory {
            items: std::mem::take(&mut self.items),
        }
    }

    /// Absorbs all stacks of `other` into this inventory.
    pub fn merge(&mut self, other: Inventory) {
        for (item, count) in other.items {
            *self.items.entry(item).or_insert(0) += count;
        }
    }
}
