use bevy::prelude::*;

use crate::zengin::{InstanceId, ScriptManager};

use super::state::{InventoryEntry, InventoryOwner, InventoryWindowState};

/// Builds the UI entry of one inventory stack from its script data.
fn inventory_entry(
    script: &mut ScriptManager,
    instance_id: InstanceId,
    count: i32,
) -> InventoryEntry {
    let symbol_name = script
        .get_name_by_id(instance_id.value())
        .unwrap_or_default();
    let is_gold = symbol_name.to_lowercase().contains("gold");
    let lowered = symbol_name.to_lowercase();
    let sort_group = if lowered.starts_with("itmw_") {
        0
    } else if lowered.starts_with("itrw_") {
        1
    } else {
        2
    };
    let name = script.item_log_name(instance_id);
    let stats = script.item_display_data(instance_id);
    let value = stats.value;
    InventoryEntry {
        instance_id,
        count,
        name,
        stats: Some(stats),
        is_gold,
        sort_group,
        value,
    }
}

/// Snapshots one npc inventory into a window.
pub(super) fn npc_inventory_window(
    script: &mut ScriptManager,
    npc: InstanceId,
    title: String,
    hide_gold: bool,
    hide_equipped: bool,
) -> InventoryWindowState {
    let mut items: Vec<InventoryEntry> = script
        .inventory(npc)
        .into_iter()
        .map(|(instance_id, count)| inventory_entry(script, instance_id, count))
        .collect();
    let gold_count = gold_sum(&items);
    let (equipped_melee, equipped_ranged) = script
        .npc_instances()
        .get(&npc)
        .map(|npc| (npc.equipped_melee, npc.equipped_ranged))
        .unwrap_or_default();
    if hide_gold {
        items.retain(|entry| !entry.is_gold);
    }
    if hide_equipped {
        items.retain(|entry| {
            Some(entry.instance_id) != equipped_melee && Some(entry.instance_id) != equipped_ranged
        });
    }
    build_window(
        InventoryOwner::Npc(npc),
        title,
        items,
        equipped_melee,
        equipped_ranged,
        gold_count,
    )
}

pub(super) fn chest_inventory_window(
    script: &mut ScriptManager,
    chest: Entity,
    title: String,
    items: &[(InstanceId, i32)],
) -> InventoryWindowState {
    let items: Vec<InventoryEntry> = items
        .iter()
        .map(|(instance_id, count)| inventory_entry(script, *instance_id, *count))
        .collect();
    let gold_count = gold_sum(&items);
    build_window(
        InventoryOwner::Chest(chest),
        title,
        items,
        None,
        None,
        gold_count,
    )
}

fn gold_sum(items: &[InventoryEntry]) -> i32 {
    items
        .iter()
        .filter(|entry| entry.is_gold)
        .map(|entry| entry.count)
        .sum()
}

fn build_window(
    owner: InventoryOwner,
    title: String,
    mut items: Vec<InventoryEntry>,
    equipped_melee: Option<InstanceId>,
    equipped_ranged: Option<InstanceId>,
    gold_count: i32,
) -> InventoryWindowState {
    sort_inventory_items(&mut items);
    InventoryWindowState {
        owner,
        title,
        selected: 0,
        equipped_melee,
        equipped_ranged,
        gold_count,
        items,
    }
}

/// Melee weapons first, then ranged, then the rest; within a group by damage
/// descending (0 for items without damage data), ties broken by instance id
/// for a deterministic order (the script inventory is unordered).
fn sort_inventory_items(items: &mut [InventoryEntry]) {
    items.sort_unstable_by(|aa, bb| {
        aa.sort_group
            .cmp(&bb.sort_group)
            .then_with(|| {
                bb.stats
                    .as_ref()
                    .map(|st| st.damage_total)
                    .unwrap_or_default()
                    .cmp(
                        &aa.stats
                            .as_ref()
                            .map(|st| st.damage_total)
                            .unwrap_or_default(),
                    )
            })
            .then_with(|| aa.instance_id.cmp(&bb.instance_id))
    });
}

/// Gold item used for payments: the known currency instance, falling back to
/// an inventory stack whose symbol name looks like gold.
pub(super) fn resolve_gold_item(
    script: &mut ScriptManager,
    player: InstanceId,
    trader: InstanceId,
) -> Option<InstanceId> {
    if let Some(gold) = script.symbol_instance("itmi_gold") {
        return Some(gold);
    }
    let stacks = script
        .inventory(player)
        .into_iter()
        .chain(script.inventory(trader));
    for (item, _count) in stacks {
        let name = script.get_name_by_id(item.value()).unwrap_or_default();
        if name.to_lowercase().contains("gold") {
            return Some(item);
        }
    }
    None
}

pub(super) fn chest_remove_item(
    items: &mut Vec<(InstanceId, i32)>,
    item: InstanceId,
    count: i32,
) -> i32 {
    let Some(index) = items.iter().position(|(entry, _)| *entry == item) else {
        return 0;
    };
    let removed = count.min(items[index].1);
    items[index].1 -= removed;
    if items[index].1 <= 0 {
        items.remove(index);
    }
    removed
}

pub(super) fn chest_add_item(items: &mut Vec<(InstanceId, i32)>, item: InstanceId, count: i32) {
    if let Some(entry) = items.iter_mut().find(|(entry, _)| *entry == item) {
        entry.1 += count;
        return;
    }
    items.push((item, count));
}

pub(super) fn clamp_selection(window: &mut Option<InventoryWindowState>, selected: usize) {
    let Some(window) = window else {
        return;
    };
    let max = window.items.len().saturating_sub(1);
    window.selected = selected.min(max);
}
