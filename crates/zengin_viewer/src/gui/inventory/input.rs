use bevy::log::{info, warn};
use bevy::prelude::*;
use leafwing_input_manager::prelude::ActionState;

use crate::gameplay::objects::PlayerCharacter;
use crate::gui::GameLog;
use crate::gui::JournalUiState;
use crate::input::{GuiAction, GuiInputMarker, SharedAction, SharedInputMarker};
use crate::zengin::DialogUiState;
use crate::zengin::{GameChest, GameNpc, InstanceId, ScriptResource, WeaponKind};

use super::data::{
    chest_add_item, chest_inventory_window, chest_remove_item, clamp_selection,
    npc_inventory_window, resolve_gold_item,
};
use super::state::{
    InventoryEntry, InventoryMode, InventoryOwner, InventoryUiState, OpenLoot, OpenTrade,
    PLAYER_WINDOW, TRADER_WINDOW,
};
use super::style::InventoryStyle;

pub(super) fn open_trade(
    mut requests: MessageReader<OpenTrade>,
    mut state: ResMut<InventoryUiState>,
    gothic_script: Res<ScriptResource>,
) {
    let Some(request) = requests.read().last() else {
        return;
    };
    let mut script = gothic_script.lock();
    let trader_title = script
        .npc_display_name(request.trader)
        .unwrap_or_else(|| script.call_get_name(request.trader.symbol()));
    let player = npc_inventory_window(
        &mut script,
        request.player,
        "Inventory".to_string(),
        true,
        true,
    );
    let trader = npc_inventory_window(&mut script, request.trader, trader_title, true, true);
    let gold_item = resolve_gold_item(&mut script, request.player, request.trader);
    let stacks = (player.items.len(), trader.items.len());
    state.windows[PLAYER_WINDOW] = Some(player);
    state.windows[TRADER_WINDOW] = Some(trader);
    state.mode = InventoryMode::Trade;
    state.active_window = TRADER_WINDOW;
    state.gold_item = gold_item;
    state.open = true;
    info!(
        "Trade opened with npc({}): player {} stack(s), trader {} stack(s)",
        request.trader.value(),
        stacks.0,
        stacks.1
    );
}

pub(super) fn open_loot(
    mut requests: MessageReader<OpenLoot>,
    mut state: ResMut<InventoryUiState>,
    gothic_script: Res<ScriptResource>,
    chests: Query<&GameChest>,
) {
    let Some(request) = requests.read().last() else {
        return;
    };
    let Ok(chest) = chests.get(request.chest) else {
        warn!("Loot request for missing chest entity({})", request.chest);
        return;
    };
    let mut script = gothic_script.lock();
    let player = npc_inventory_window(
        &mut script,
        request.player,
        "Inventory".to_string(),
        false,
        true,
    );
    let chest_window = chest_inventory_window(
        &mut script,
        request.chest,
        "Chest".to_string(),
        &chest.items,
    );
    let stacks = (player.items.len(), chest_window.items.len());
    state.windows[PLAYER_WINDOW] = Some(player);
    state.windows[TRADER_WINDOW] = Some(chest_window);
    state.mode = InventoryMode::Loot;
    state.active_window = TRADER_WINDOW;
    state.gold_item = None;
    state.open = true;
    info!(
        "Loot opened for chest({}) vob({}): player {} stack(s), chest {} stack(s)",
        request.chest, chest.name, stacks.0, stacks.1
    );
}

/// The dialog UI takes precedence: no inventory while a dialog or the journal
/// is open.
pub(super) fn toggle_inventory(
    action_state: Single<&ActionState<SharedAction>, With<SharedInputMarker>>,
    dialog_ui: Res<DialogUiState>,
    journal_ui: Res<JournalUiState>,
    mut state: ResMut<InventoryUiState>,
    gothic_script: Res<ScriptResource>,
    player_npc: Query<&GameNpc, With<PlayerCharacter>>,
) {
    if !action_state.just_pressed(&SharedAction::Inventory) {
        return;
    }
    if state.open {
        state.close();
        return;
    }
    if dialog_ui.is_open() || journal_ui.is_open() {
        return;
    }

    let Ok(player_npc) = player_npc.single() else {
        return;
    };
    let player_id = player_npc.instance_id();
    let mut script = gothic_script.lock();
    let window = npc_inventory_window(
        &mut script,
        player_id,
        "Inventory".to_string(),
        false,
        false,
    );
    let stacks = window.items.len();
    state.windows[PLAYER_WINDOW] = Some(window);
    state.windows[TRADER_WINDOW] = None;
    state.mode = InventoryMode::Player;
    state.active_window = PLAYER_WINDOW;
    state.gold_item = None;
    state.open = true;
    info!("Inventory opened with {stacks} item stack(s)");
}

/// Grid navigation and closing while the inventory is open. When a second
/// window is shown, moving past the left/right edge switches windows; confirm
/// applies the action of the focused window for the current mode.
pub(super) fn handle_inventory_input(
    action_state: Single<&ActionState<GuiAction>, With<GuiInputMarker>>,
    mut state: ResMut<InventoryUiState>,
    mut game_log: ResMut<GameLog>,
    gothic_script: Res<ScriptResource>,
    player_npc: Query<&GameNpc, With<PlayerCharacter>>,
    mut chests: Query<&mut GameChest>,
) {
    if !state.open {
        return;
    }
    if action_state.just_pressed(&GuiAction::CancelAction) {
        state.close();
        return;
    }

    let multi_window = state.mode != InventoryMode::Player;
    let active = state.active_window;
    let columns = InventoryStyle::COLUMNS;
    let selected = state.windows[active]
        .as_ref()
        .map_or(0, |window| window.selected);
    let slot_count = state.windows[active].as_ref().map_or(0, |window| {
        window.items.len().min(columns * InventoryStyle::ROWS)
    });

    if action_state.just_pressed(&GuiAction::Left) {
        if multi_window && active == PLAYER_WINDOW && selected.is_multiple_of(columns) {
            state.active_window = TRADER_WINDOW;
        } else if !selected.is_multiple_of(columns)
            && let Some(window) = state.windows[active].as_mut()
        {
            window.selected -= 1;
        }
    }
    if action_state.just_pressed(&GuiAction::Right) {
        let at_right_edge = selected + 1 >= slot_count || selected % columns + 1 >= columns;
        if multi_window && active == TRADER_WINDOW && at_right_edge {
            state.active_window = PLAYER_WINDOW;
        } else if selected + 1 < slot_count
            && selected % columns + 1 < columns
            && let Some(window) = state.windows[active].as_mut()
        {
            window.selected += 1;
        }
    }
    if action_state.just_pressed(&GuiAction::Up)
        && selected >= columns
        && let Some(window) = state.windows[active].as_mut()
    {
        window.selected -= columns;
    }
    if action_state.just_pressed(&GuiAction::Down)
        && selected + columns < slot_count
        && let Some(window) = state.windows[active].as_mut()
    {
        window.selected += columns;
    }

    if !action_state.just_pressed(&GuiAction::Confirm) {
        return;
    }
    match state.mode {
        InventoryMode::Trade => trade_confirm(&mut state, active, &mut game_log, &gothic_script),
        InventoryMode::Loot => {
            loot_confirm(
                &mut state,
                active,
                &mut game_log,
                &gothic_script,
                &mut chests,
            );
        }
        InventoryMode::Player => {
            if let Some(window) = state.windows[active].as_mut()
                && let Some(entry) = window.items.get(window.selected)
            {
                let (equipped_melee, equipped_ranged) =
                    equip_selected_item(entry, &mut game_log, &gothic_script, player_npc);
                window.equipped_melee = equipped_melee;
                window.equipped_ranged = equipped_ranged;
            }
        }
    }
}

/// Trade panels belong to an open dialog, closing the dialog closes them too.
pub(super) fn close_orphaned_trade(
    dialog_ui: Res<DialogUiState>,
    mut state: ResMut<InventoryUiState>,
) {
    if state.is_trade() && !dialog_ui.is_open() {
        state.close();
    }
}

/// Buys one item from the trader window or sells one from the player window,
/// then re-snapshots both windows so counts and gold stay current.
fn trade_confirm(
    state: &mut InventoryUiState,
    active: usize,
    game_log: &mut GameLog,
    gothic_script: &ScriptResource,
) {
    let Some(window) = state.windows[active].as_ref() else {
        return;
    };
    let Some(entry) = window.items.get(window.selected) else {
        return;
    };
    let item = entry.instance_id;
    let item_name = entry.name.clone();
    let buying = active == TRADER_WINDOW;
    let price = if buying {
        entry.buy_price()
    } else {
        entry.sell_price()
    };

    let (Some(player), Some(trader)) = (
        state.windows[PLAYER_WINDOW]
            .as_ref()
            .and_then(|window| match window.owner {
                InventoryOwner::Npc(npc) => Some(npc),
                InventoryOwner::Chest(_) => None,
            }),
        state.windows[TRADER_WINDOW]
            .as_ref()
            .and_then(|window| match window.owner {
                InventoryOwner::Npc(npc) => Some(npc),
                InventoryOwner::Chest(_) => None,
            }),
    ) else {
        return;
    };
    if buying {
        let player_gold = state.windows[PLAYER_WINDOW]
            .as_ref()
            .map_or(0, |window| window.gold_count);
        if player_gold < price {
            game_log.add(format!("Not enough gold for {item_name} ({price})"));
            return;
        }
    }
    let gold_item = state.gold_item;
    if price > 0 && gold_item.is_none() {
        warn!("Trade has no gold item, {item_name} was not traded");
        return;
    }

    let mut script = gothic_script.lock();
    let (from, to) = if buying {
        (trader, player)
    } else {
        (player, trader)
    };
    if script.remove_inventory(from, item, 1) == 0 {
        warn!(
            "Trade item({}) is no longer available from npc({})",
            item.value(),
            from.value()
        );
        return;
    }
    script.add_inventory(to, item, 1);
    if price > 0
        && let Some(gold_item) = gold_item
    {
        if buying {
            script.remove_inventory(player, gold_item, price);
            script.add_inventory(trader, gold_item, price);
        } else {
            script.add_inventory(player, gold_item, price);
        }
    }
    if buying {
        game_log.add(format!("Bought {item_name} for {price} gold"));
    } else {
        game_log.add(format!("Sold {item_name} for {price} gold"));
    }

    let player_selected = state.windows[PLAYER_WINDOW]
        .as_ref()
        .map_or(0, |window| window.selected);
    let trader_selected = state.windows[TRADER_WINDOW]
        .as_ref()
        .map_or(0, |window| window.selected);
    let trader_title = state.windows[TRADER_WINDOW]
        .as_ref()
        .map(|window| window.title.clone())
        .unwrap_or_default();
    state.windows[PLAYER_WINDOW] = Some(npc_inventory_window(
        &mut script,
        player,
        "Inventory".to_string(),
        true,
        true,
    ));
    state.windows[TRADER_WINDOW] = Some(npc_inventory_window(
        &mut script,
        trader,
        trader_title,
        true,
        true,
    ));
    clamp_selection(&mut state.windows[PLAYER_WINDOW], player_selected);
    clamp_selection(&mut state.windows[TRADER_WINDOW], trader_selected);
}

/// Moves one item between the chest window and the player window for free, then
/// re-snapshots both windows so counts stay current.
#[allow(clippy::cognitive_complexity)]
fn loot_confirm(
    state: &mut InventoryUiState,
    active: usize,
    game_log: &mut GameLog,
    gothic_script: &ScriptResource,
    chests: &mut Query<&mut GameChest>,
) {
    let Some(window) = state.windows[active].as_ref() else {
        return;
    };
    let Some(entry) = window.items.get(window.selected) else {
        return;
    };
    let item = entry.instance_id;
    let item_name = entry.name.clone();

    let Some(player) =
        state.windows[PLAYER_WINDOW]
            .as_ref()
            .and_then(|window| match window.owner {
                InventoryOwner::Npc(npc) => Some(npc),
                InventoryOwner::Chest(_) => None,
            })
    else {
        return;
    };
    let Some(chest_entity) = state.windows[TRADER_WINDOW]
        .as_ref()
        .and_then(|window| match window.owner {
            InventoryOwner::Chest(chest) => Some(chest),
            InventoryOwner::Npc(_) => None,
        })
    else {
        return;
    };
    let Ok(mut chest) = chests.get_mut(chest_entity) else {
        warn!("Loot chest entity({chest_entity}) is missing");
        return;
    };

    let taking = active == TRADER_WINDOW;
    let mut script = gothic_script.lock();
    if taking {
        if chest_remove_item(&mut chest.items, item, 1) == 0 {
            warn!(
                "Loot item({}) is no longer available in chest({chest_entity})",
                item.value()
            );
            return;
        }
        script.add_inventory(player, item, 1);
        game_log.add(format!("Took {item_name}"));
        info!(
            "Player({}) took item({}) from chest({chest_entity})",
            player.value(),
            item.value()
        );
    } else {
        if script.remove_inventory(player, item, 1) == 0 {
            warn!(
                "Loot item({}) is no longer available from player({})",
                item.value(),
                player.value()
            );
            return;
        }
        chest_add_item(&mut chest.items, item, 1);
        game_log.add(format!("Stored {item_name}"));
        info!(
            "Player({}) stored item({}) in chest({chest_entity})",
            player.value(),
            item.value()
        );
    }

    let player_selected = state.windows[PLAYER_WINDOW]
        .as_ref()
        .map_or(0, |window| window.selected);
    let chest_selected = state.windows[TRADER_WINDOW]
        .as_ref()
        .map_or(0, |window| window.selected);
    let chest_title = state.windows[TRADER_WINDOW]
        .as_ref()
        .map(|window| window.title.clone())
        .unwrap_or_default();
    state.windows[PLAYER_WINDOW] = Some(npc_inventory_window(
        &mut script,
        player,
        "Inventory".to_string(),
        false,
        true,
    ));
    state.windows[TRADER_WINDOW] = Some(chest_inventory_window(
        &mut script,
        chest_entity,
        chest_title,
        &chest.items,
    ));
    clamp_selection(&mut state.windows[PLAYER_WINDOW], player_selected);
    clamp_selection(&mut state.windows[TRADER_WINDOW], chest_selected);
}

/// Equips the selected item as the player's weapon of its kind on confirm, a
/// second confirm unequips it again. A melee and a ranged weapon can be
/// equipped at the same time, equipping again replaces the same slot. Returns
/// the items equipped afterwards.
fn equip_selected_item(
    entry: &InventoryEntry,
    game_log: &mut GameLog,
    gothic_script: &ScriptResource,
    player_npc: Query<&GameNpc, With<PlayerCharacter>>,
) -> (Option<InstanceId>, Option<InstanceId>) {
    let Ok(player_npc) = player_npc.single() else {
        return (None, None);
    };
    let player_id = player_npc.instance_id();

    let mut script = gothic_script.lock();
    let (equipped_melee, equipped_ranged) = script
        .npc_instances()
        .get(&player_id)
        .map(|npc| (npc.equipped_melee, npc.equipped_ranged))
        .unwrap_or_default();

    let symbol = script
        .get_name_by_id(entry.instance_id.value())
        .unwrap_or_default()
        .to_lowercase();
    let Some(kind) = WeaponKind::from_symbol_name(&symbol) else {
        game_log.add(format!("{} is not a weapon", entry.name));
        return (equipped_melee, equipped_ranged);
    };

    let currently = match kind {
        WeaponKind::Melee => equipped_melee,
        WeaponKind::Bow | WeaponKind::Crossbow => equipped_ranged,
    };
    if currently == Some(entry.instance_id) {
        script.unequip_weapon_kind(player_id, kind);
        game_log.add(format!("Unequipped {}", entry.name));
        info!(
            "Player({}) unequipped item({})",
            player_id.value(),
            entry.instance_id.value()
        );
    } else {
        script.equip_item(player_id, entry.instance_id);
        game_log.add(format!("Equipped {}", entry.name));
        info!(
            "Player({}) equipped item({}) sheathed",
            player_id.value(),
            entry.instance_id.value()
        );
    }

    script
        .npc_instances()
        .get(&player_id)
        .map(|npc| (npc.equipped_melee, npc.equipped_ranged))
        .unwrap_or_default()
}
