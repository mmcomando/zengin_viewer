use bevy::prelude::*;

use crate::ui_kit::{set_display, set_text, set_visibility};

use super::spawn::{
    InventoryContainer, InventoryInfoText, InventoryInfoValues, InventorySlot,
    InventorySlotEquippedOverlay, InventorySlotTextOrCount, InventoryTextures,
    InventoryWindowGoldText, InventoryWindowPanel, InventoryWindowTitleText,
};
use super::state::{InventoryEntry, InventoryUiState};
use super::style::InventoryStyle;

/// Updates visibility, header texts, slot contents and the info panel from
/// the state.
#[allow(clippy::type_complexity, clippy::cognitive_complexity)]
pub(super) fn sync_inventory_ui(
    state: Res<InventoryUiState>,
    textures: Res<InventoryTextures>,
    mut containers: Query<(&InventoryContainer, &mut Visibility)>,
    mut panels: Query<(&InventoryWindowPanel, &mut Node)>,
    mut slots: Query<
        (&InventorySlot, &mut Node, &mut ImageNode, &Children),
        (
            Without<InventorySlotEquippedOverlay>,
            Without<InventoryWindowPanel>,
        ),
    >,
    mut equipped_overlays: Query<
        &mut Node,
        (
            With<InventorySlotEquippedOverlay>,
            Without<InventoryWindowPanel>,
        ),
    >,
    mut slot_texts: Query<
        (&mut Text, &InventorySlotTextOrCount),
        (
            Without<InventoryInfoText>,
            Without<InventoryWindowTitleText>,
            Without<InventoryWindowGoldText>,
        ),
    >,
    mut window_title: Query<
        (&InventoryWindowTitleText, &mut Text),
        (
            Without<InventoryInfoText>,
            Without<InventorySlotTextOrCount>,
            Without<InventoryWindowGoldText>,
        ),
    >,
    mut window_gold: Query<
        (&InventoryWindowGoldText, &mut Text),
        (
            Without<InventoryInfoText>,
            Without<InventorySlotTextOrCount>,
            Without<InventoryWindowTitleText>,
        ),
    >,
    mut info_text: Query<
        (&mut Text, Has<InventoryInfoValues>),
        (
            With<InventoryInfoText>,
            Without<InventorySlotTextOrCount>,
            Without<InventoryWindowTitleText>,
            Without<InventoryWindowGoldText>,
        ),
    >,
) {
    for (_, mut visibility) in containers.iter_mut() {
        set_visibility(&mut visibility, state.open);
    }
    if !state.open {
        return;
    }

    for (panel, mut node) in panels.iter_mut() {
        let used = state
            .windows
            .get(panel.0)
            .is_some_and(|window| window.is_some());
        set_display(&mut node, used);
    }

    for (marker, mut text) in window_title.iter_mut() {
        let Some(window) = state.windows.get(marker.0).and_then(Option::as_ref) else {
            continue;
        };
        set_text(&mut text, &window.title);
    }
    for (marker, mut text) in window_gold.iter_mut() {
        let Some(window) = state.windows.get(marker.0).and_then(Option::as_ref) else {
            continue;
        };
        let gold = format!("Gold: {}", window.gold_count);
        set_text(&mut text, &gold);
    }

    for (slot, mut node, mut image, children) in slots.iter_mut() {
        let Some(window) = state.windows.get(slot.window).and_then(Option::as_ref) else {
            set_display(&mut node, false);
            continue;
        };
        let slot_count = window
            .items
            .len()
            .min(InventoryStyle::COLUMNS * InventoryStyle::ROWS);
        let used = slot.index < slot_count;
        set_display(&mut node, used);
        if !used {
            continue;
        }
        let entry = &window.items[slot.index];
        let is_equipped = window.equipped_melee == Some(entry.instance_id)
            || window.equipped_ranged == Some(entry.instance_id);
        let focused = slot.window == state.active_window && slot.index == window.selected;
        let texture = if focused {
            &textures.focus
        } else {
            &textures.slot
        };
        if image.image != *texture {
            image.image = texture.clone();
        }

        let display_text: String = entry
            .name
            .chars()
            .take(InventoryStyle::SLOT_NAME_CHARS)
            .collect();
        let count_text = if entry.count > 1 {
            entry.count.to_string()
        } else {
            String::new()
        };
        for &child in children {
            if let Ok(mut overlay) = equipped_overlays.get_mut(child) {
                set_display(&mut overlay, is_equipped);
                continue;
            }
            if let Ok((mut text, kind)) = slot_texts.get_mut(child) {
                let desired = match kind {
                    InventorySlotTextOrCount::Name => &display_text,
                    InventorySlotTextOrCount::Count => &count_text,
                };
                set_text(&mut text, desired);
            }
        }
    }

    let (labels, values) = state.windows[state.active_window]
        .as_ref()
        .and_then(|window| window.items.get(window.selected))
        .map(|entry| item_info_text(entry, state.is_trade()))
        .unwrap_or_default();
    for (mut text, is_values) in info_text.iter_mut() {
        let info = if is_values { &values } else { &labels };
        set_text(&mut text, info);
    }
}
/// Item info panel content: item name, flavor description and the generic
/// script info-box lines. Returns (labels, values) with one line per row
/// so the two text columns line up.
fn item_info_text(entry: &InventoryEntry, is_trade: bool) -> (String, String) {
    let mut label_lines: Vec<String> = vec![entry.name.clone(), String::new()];
    let mut value_lines: Vec<String> = vec![String::new(), String::new()];
    if is_trade {
        label_lines.push("Buy:".to_string());
        value_lines.push(entry.buy_price().to_string());
        label_lines.push("Sell:".to_string());
        value_lines.push(entry.sell_price().to_string());
    } else {
        label_lines.push("Value:".to_string());
        value_lines.push(entry.buy_price().to_string());
    }
    if let Some(stats) = &entry.stats {
        // The description often just repeats the name, skip it in that case
        if let Some(description) = &stats.description
            && !description.eq_ignore_ascii_case(&entry.name)
        {
            label_lines.push(description.clone());
            value_lines.push(String::new());
        }
        for (name, value) in &stats.requirements {
            label_lines.push(format!("Required {name}:"));
            value_lines.push(value.to_string());
        }
        for (text, count) in &stats.info_lines {
            // Label-only script lines have a count of 0 and show no value
            label_lines.push(text.clone());
            value_lines.push(if *count == 0 {
                String::new()
            } else {
                count.to_string()
            });
        }
    }
    // Joined without a trailing newline so the last row sits at the bottom of
    // the panel and the two columns line up row by row
    (label_lines.join("\n"), value_lines.join("\n"))
}
