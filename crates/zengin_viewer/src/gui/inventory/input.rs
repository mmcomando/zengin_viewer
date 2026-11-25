use bevy::log::{info, warn};
use bevy::prelude::*;
use leafwing_input_manager::prelude::ActionState;

use crate::audio::PlaySound;
use crate::gameplay::objects::PlayerCharacter;
use crate::gui::GameLog;
use crate::gui::JournalUiState;
use crate::gui::StatsUiState;
use crate::input::{GuiAction, GuiInputMarker, SharedAction, SharedInputMarker};
use crate::perf_debug;
use crate::test_log;
use crate::zengin::DialogUiState;
use crate::zengin::{
    GameChest, GameNpc, InstanceId, MobBase, ScriptManager, ScriptResource, WeaponKind, ZenginVfs,
    resolve_sound_asset,
};

use super::data::{
    chest_add_item, chest_inventory_window, chest_remove_item, clamp_selection,
    corpse_inventory_window, npc_inventory_window, resolve_gold_item,
};
use super::state::{
    InventoryEntry, InventoryMode, InventoryOwner, InventoryUiState, InventoryWindowState,
    LockpickOpened, LockpickSession, LootClosed, LootSource, OpenLockpick, OpenLoot, OpenTrade,
    PLAYER_WINDOW, TRADER_WINDOW,
};
use super::style::InventoryStyle;

pub(super) fn open_trade(
    mut requests: MessageReader<OpenTrade>,
    mut state: ResMut<InventoryUiState>,
    gothic_script: Res<ScriptResource>,
) {
    let _perf = perf_debug!();
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
    chests: Query<(&GameChest, &MobBase)>,
    npcs: Query<&GameNpc>,
) {
    let _perf = perf_debug!();
    let Some(request) = requests.read().last() else {
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
    let left = match request.source {
        LootSource::Chest(entity) => open_chest_loot_window(&mut script, entity, &chests),
        LootSource::Corpse(entity) => open_corpse_loot_window(&mut script, entity, &npcs),
    };
    let Some(left) = left else {
        return;
    };
    state.windows[PLAYER_WINDOW] = Some(player);
    state.windows[TRADER_WINDOW] = Some(left);
    state.mode = InventoryMode::Loot;
    state.active_window = TRADER_WINDOW;
    state.gold_item = None;
    state.open = true;
    state.loot_source = Some(request.source);
    state.lockpick = None;
}

/// Opens the lockpick sequence for a locked chest, the player is already
/// kneeling at it.
pub(super) fn open_lockpick(
    mut requests: MessageReader<OpenLockpick>,
    mut state: ResMut<InventoryUiState>,
    chests: Query<&GameChest>,
    mut closed: MessageWriter<LootClosed>,
) {
    let _perf = perf_debug!();
    let Some(request) = requests.read().last() else {
        return;
    };
    let Ok(chest) = chests.get(request.chest) else {
        return;
    };
    let session = LockpickSession::new(request.chest, &chest.pick_string);
    if session.sequence.is_empty() {
        warn!(
            "Chest({}) pick_string({:?}) has no L/R steps",
            request.chest, chest.pick_string
        );
        closed.write(LootClosed {
            source: LootSource::Chest(request.chest),
        });
        return;
    }
    state.open = true;
    state.mode = InventoryMode::LockPick;
    state.windows = Default::default();
    state.active_window = PLAYER_WINDOW;
    state.gold_item = None;
    state.lockpick = Some(session);
    state.loot_source = Some(LootSource::Chest(request.chest));
}

/// Advances the lockpick sequence on `Left`/`Right` input; a wrong step resets
/// the progress.
pub(super) fn handle_lockpick_input(
    action_state: Single<&ActionState<GuiAction>, With<GuiInputMarker>>,
    mut state: ResMut<InventoryUiState>,
    gothic_script: Res<ScriptResource>,
    vfs: Res<ZenginVfs>,
    mut game_log: ResMut<GameLog>,
    mut sounds: MessageWriter<PlaySound>,
    mut opened: MessageWriter<LockpickOpened>,
) {
    let _perf = perf_debug!();
    if state.mode != InventoryMode::LockPick {
        return;
    }
    let step = if action_state.just_pressed(&GuiAction::Left) {
        Some('L')
    } else if action_state.just_pressed(&GuiAction::Right) {
        Some('R')
    } else {
        None
    };
    let Some(step) = step else {
        return;
    };
    let Some(session) = state.lockpick.as_mut() else {
        return;
    };
    let expected = session.sequence.get(session.progress).copied();
    if expected != Some(step) {
        session.progress = 0;
        play_lockpick_sound(&vfs, &mut sounds, "PICKLOCK_FAILURE");
        game_log.add(lockpick_text(
            &gothic_script,
            "print_picklock_failure",
            "The lock resists.",
        ));
        return;
    }
    session.progress += 1;
    let chest = session.chest;
    if session.progress < session.sequence.len() {
        play_lockpick_sound(&vfs, &mut sounds, "PICKLOCK_SUCCESS");
        game_log.add(lockpick_text(
            &gothic_script,
            "print_picklock_success",
            "The pick slides into place.",
        ));
        return;
    }
    state.lockpick = None;
    play_lockpick_sound(&vfs, &mut sounds, "PICKLOCK_UNLOCK");
    game_log.add(lockpick_text(
        &gothic_script,
        "print_picklock_unlock",
        "The lock clicks open.",
    ));
    opened.write(LockpickOpened { chest });
}

/// The dialog UI takes precedence: no inventory while a dialog or the journal
/// is open.
pub(super) fn toggle_inventory(
    action_state: Single<&ActionState<SharedAction>, With<SharedInputMarker>>,
    dialog_ui: Res<DialogUiState>,
    journal_ui: Res<JournalUiState>,
    stats_ui: Res<StatsUiState>,
    mut state: ResMut<InventoryUiState>,
    gothic_script: Res<ScriptResource>,
    player_npc: Query<&GameNpc, With<PlayerCharacter>>,
) {
    let _perf = perf_debug!();
    if !action_state.just_pressed(&SharedAction::Inventory) {
        return;
    }
    if state.open {
        state.close();
        return;
    }
    if dialog_ui.is_open() || journal_ui.is_open() || stats_ui.is_open() {
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
/// window is shown, moving past the left/right edge switches windows; the
/// Confirm key (left mouse button, left ctrl) applies the action of the focused
/// window for the current mode.
pub(super) fn handle_inventory_input(
    action_state: Single<&ActionState<GuiAction>, With<GuiInputMarker>>,
    mut state: ResMut<InventoryUiState>,
    mut game_log: ResMut<GameLog>,
    gothic_script: Res<ScriptResource>,
    player_npc: Query<&GameNpc, With<PlayerCharacter>>,
    mut chests: Query<&mut GameChest>,
) {
    let _perf = perf_debug!();
    if !state.open {
        return;
    }
    if action_state.just_pressed(&GuiAction::CancelAction) {
        state.close();
        return;
    }
    if state.mode == InventoryMode::LockPick {
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
                let (equipped_melee, equipped_ranged, equipped_armor) =
                    equip_selected_item(entry, &mut game_log, &gothic_script, player_npc);
                window.equipped_melee = equipped_melee;
                window.equipped_ranged = equipped_ranged;
                window.equipped_armor = equipped_armor;
            }
        }
        InventoryMode::LockPick => {}
    }
}

/// Emits the loot-close event when the panels leave the open state.
pub(super) fn emit_loot_closed(
    mut state: ResMut<InventoryUiState>,
    mut closed: MessageWriter<LootClosed>,
) {
    let _perf = perf_debug!();
    if state.open {
        return;
    }
    let Some(source) = state.loot_source.take() else {
        return;
    };
    closed.write(LootClosed { source });
}

/// Trade panels belong to an open dialog, closing the dialog closes them too.
pub(super) fn close_orphaned_trade(
    dialog_ui: Res<DialogUiState>,
    mut state: ResMut<InventoryUiState>,
) {
    let _perf = perf_debug!();
    if state.is_trade() && !dialog_ui.is_open() {
        state.close();
    }
}

/// Builds the left loot window of a chest, `None` when the entity is gone.
fn open_chest_loot_window(
    script: &mut ScriptManager,
    chest_entity: Entity,
    chests: &Query<(&GameChest, &MobBase)>,
) -> Option<InventoryWindowState> {
    let Ok((chest, base)) = chests.get(chest_entity) else {
        warn!("Loot request for missing chest entity({chest_entity})");
        return None;
    };
    let title = script.mob_focus_name(&base.focus_name);
    let window = chest_inventory_window(script, title, &chest.items);
    Some(window)
}

/// Builds the left loot window of a corpse, `None` when the entity is gone.
fn open_corpse_loot_window(
    script: &mut ScriptManager,
    corpse_entity: Entity,
    npcs: &Query<&GameNpc>,
) -> Option<InventoryWindowState> {
    let Ok(npc) = npcs.get(corpse_entity) else {
        warn!("Loot request for missing corpse entity({corpse_entity})");
        return None;
    };
    let npc_id = npc.instance_id();
    let title = script
        .npc_display_name(npc_id)
        .unwrap_or_else(|| script.call_get_name(npc_id.symbol()));
    let window = corpse_inventory_window(script, npc_id, title);
    Some(window)
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
                InventoryOwner::Chest => None,
            }),
        state.windows[TRADER_WINDOW]
            .as_ref()
            .and_then(|window| match window.owner {
                InventoryOwner::Npc(npc) => Some(npc),
                InventoryOwner::Chest => None,
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
        .map_or_default(|window| window.title.clone());
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

/// Applies the focused window's loot action, then re-snapshots both windows so
/// counts stay current.
fn loot_confirm(
    state: &mut InventoryUiState,
    active: usize,
    game_log: &mut GameLog,
    gothic_script: &ScriptResource,
    chests: &mut Query<&mut GameChest>,
) {
    let Some(source) = state.loot_source else {
        return;
    };
    match source {
        LootSource::Chest(chest) => {
            chest_loot_confirm(state, active, chest, game_log, gothic_script, chests);
        }
        LootSource::Corpse(corpse) => {
            corpse_loot_confirm(state, active, corpse, game_log, gothic_script);
        }
    }
}

/// Moves one item between the chest window and the player window for free.
#[allow(clippy::cognitive_complexity)]
fn chest_loot_confirm(
    state: &mut InventoryUiState,
    active: usize,
    chest_entity: Entity,
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

    let Some(player) = window_npc(state.windows[PLAYER_WINDOW].as_ref()) else {
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
        .map_or_default(|window| window.title.clone());
    state.windows[PLAYER_WINDOW] = Some(npc_inventory_window(
        &mut script,
        player,
        "Inventory".to_string(),
        false,
        true,
    ));
    state.windows[TRADER_WINDOW] = Some(chest_inventory_window(
        &mut script,
        chest_title,
        &chest.items,
    ));
    clamp_selection(&mut state.windows[PLAYER_WINDOW], player_selected);
    clamp_selection(&mut state.windows[TRADER_WINDOW], chest_selected);
}

/// Takes one item from the corpse; storing back into a corpse is not offered.
fn corpse_loot_confirm(
    state: &mut InventoryUiState,
    active: usize,
    corpse_entity: Entity,
    game_log: &mut GameLog,
    gothic_script: &ScriptResource,
) {
    if active != TRADER_WINDOW {
        return;
    }
    let Some(window) = state.windows[TRADER_WINDOW].as_ref() else {
        return;
    };
    let Some(entry) = window.items.get(window.selected) else {
        return;
    };
    let item = entry.instance_id;
    let item_name = entry.name.clone();

    let Some(corpse) = window_npc(state.windows[TRADER_WINDOW].as_ref()) else {
        return;
    };
    let Some(player) = window_npc(state.windows[PLAYER_WINDOW].as_ref()) else {
        return;
    };

    let mut script = gothic_script.lock();
    if script.remove_inventory(corpse, item, 1) == 0 {
        warn!(
            "Loot item({}) is no longer available on corpse({corpse_entity})",
            item.value()
        );
        return;
    }
    script.add_inventory(player, item, 1);
    game_log.add(format!("Took {item_name}"));
    test_log!(
        "Player({}) took item({}) from corpse({corpse_entity}) npc({})",
        player.value(),
        item.value(),
        corpse.value()
    );

    let player_selected = state.windows[PLAYER_WINDOW]
        .as_ref()
        .map_or(0, |window| window.selected);
    let corpse_selected = state.windows[TRADER_WINDOW]
        .as_ref()
        .map_or(0, |window| window.selected);
    let corpse_title = state.windows[TRADER_WINDOW]
        .as_ref()
        .map_or_default(|window| window.title.clone());
    state.windows[PLAYER_WINDOW] = Some(npc_inventory_window(
        &mut script,
        player,
        "Inventory".to_string(),
        false,
        true,
    ));
    state.windows[TRADER_WINDOW] = Some(corpse_inventory_window(&mut script, corpse, corpse_title));
    clamp_selection(&mut state.windows[PLAYER_WINDOW], player_selected);
    clamp_selection(&mut state.windows[TRADER_WINDOW], corpse_selected);
}

fn window_npc(window: Option<&InventoryWindowState>) -> Option<InstanceId> {
    match window?.owner {
        InventoryOwner::Npc(npc) => Some(npc),
        InventoryOwner::Chest => None,
    }
}

/// Equips the selected item as the player's weapon of its kind on confirm, a
/// second confirm unequips it again. A melee and a ranged weapon can be
/// equipped at the same time, equipping again replaces the same slot. Returns
/// the items equipped afterwards.
#[allow(clippy::cognitive_complexity)]
fn equip_selected_item(
    entry: &InventoryEntry,
    game_log: &mut GameLog,
    gothic_script: &ScriptResource,
    player_npc: Query<&GameNpc, With<PlayerCharacter>>,
) -> (Option<InstanceId>, Option<InstanceId>, Option<InstanceId>) {
    let Ok(player_npc) = player_npc.single() else {
        return (None, None, None);
    };
    let player_id = player_npc.instance_id();

    let mut script = gothic_script.lock();
    let (equipped_melee, equipped_ranged, equipped_armor) =
        script.npc_state(player_id).map_or_default(|npc| {
            (
                npc.equipment.melee,
                npc.equipment.ranged,
                npc.equipment.armor,
            )
        });

    let symbol = script
        .get_name_by_id(entry.instance_id.value())
        .unwrap_or_default()
        .to_lowercase();
    if script.item_is_armor(entry.instance_id) {
        if equipped_armor == Some(entry.instance_id) {
            script.unequip_armor(player_id);
            game_log.add(format!("Removed {}", entry.name));
            info!(
                "Player({}) removed armor({})",
                player_id.value(),
                entry.instance_id.value()
            );
        } else {
            script.equip_item(player_id, entry.instance_id);
            game_log.add(format!("Equipped {}", entry.name));
            info!(
                "Player({}) equipped armor({})",
                player_id.value(),
                entry.instance_id.value()
            );
        }
        return script.npc_state(player_id).map_or_default(|npc| {
            (
                npc.equipment.melee,
                npc.equipment.ranged,
                npc.equipment.armor,
            )
        });
    }
    let Some(kind) = WeaponKind::from_symbol_name(&symbol) else {
        game_log.add(format!("{} is not a weapon or armor", entry.name));
        return (equipped_melee, equipped_ranged, equipped_armor);
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

    script.npc_state(player_id).map_or_default(|npc| {
        (
            npc.equipment.melee,
            npc.equipment.ranged,
            npc.equipment.armor,
        )
    })
}

fn play_lockpick_sound(vfs: &ZenginVfs, sounds: &mut MessageWriter<PlaySound>, name: &str) {
    let Some(asset_path) = resolve_sound_asset(vfs, name) else {
        warn!("Lockpick sound({name}) not found");
        return;
    };
    sounds.write(PlaySound {
        asset_path,
        position: None,
    });
}

fn lockpick_text(script: &ScriptResource, symbol: &str, fallback: &str) -> String {
    script
        .lock()
        .string_variable(symbol)
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| fallback.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loot_close_is_emitted_once() {
        let mut app = App::new();
        app.add_message::<LootClosed>();
        app.init_resource::<InventoryUiState>();
        app.add_systems(Update, emit_loot_closed);

        let chest = Entity::PLACEHOLDER;
        {
            let mut state = app.world_mut().resource_mut::<InventoryUiState>();
            state.open = true;
            state.loot_source = Some(LootSource::Chest(chest));
        }
        app.update();
        assert_eq!(
            app.world().resource::<Messages<LootClosed>>().len(),
            0,
            "no close while the panels stay open"
        );

        app.world_mut().resource_mut::<InventoryUiState>().open = false;
        app.update();
        assert_eq!(
            app.world().resource::<Messages<LootClosed>>().len(),
            1,
            "closing the loot panels emits one event"
        );
        assert!(
            app.world()
                .resource::<InventoryUiState>()
                .loot_source
                .is_none()
        );

        app.update();
        app.update();
        assert_eq!(
            app.world().resource::<Messages<LootClosed>>().len(),
            0,
            "the event is not repeated"
        );
    }
}
