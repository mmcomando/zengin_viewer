use bevy::prelude::*;

use crate::zengin::{InstanceId, ItemDisplayData};

/// One entry shown in a grid: an inventory stack with its display name.
pub(crate) struct InventoryEntry {
    pub(crate) instance_id: InstanceId,
    pub(crate) count: i32,
    pub(crate) name: String,
    /// Stats shown in the info panel, `None` when the item has no script data.
    pub(crate) stats: Option<ItemDisplayData>,
    /// Whether the item is a gold stack, based on the script symbol name so
    /// localized display names do not break the gold sum.
    pub(crate) is_gold: bool,
    /// Sort group: melee weapon, ranged weapon, everything else.
    pub(crate) sort_group: u8,
    /// Base value (`C_Item.value`), the buy price.
    pub(crate) value: i32,
}

impl InventoryEntry {
    /// Price the player pays for this item.
    pub(crate) fn buy_price(&self) -> i32 {
        self.value.max(0)
    }

    /// Price the player receives for this item.
    pub(crate) fn sell_price(&self) -> i32 {
        (self.value.max(0) as f32 * TRADE_VALUE_MULTIPLIER).ceil() as i32
    }
}

/// One window's state: open panels snapshot their items on open.
pub(crate) struct InventoryWindowState {
    /// Inventory source behind this window.
    pub(crate) owner: InventoryOwner,
    /// Header title, e.g. a category name or the trader name.
    pub(crate) title: String,
    pub(crate) selected: usize,
    /// Currently equipped weapons of this window's npc, shown with a red slot.
    pub(crate) equipped_melee: Option<InstanceId>,
    pub(crate) equipped_ranged: Option<InstanceId>,
    /// Gold stacks are not always shown in the grid, the header always needs them.
    pub(crate) gold_count: i32,
    pub(crate) items: Vec<InventoryEntry>,
}

/// Source of the items shown by one inventory window.
#[derive(Clone, Copy)]
pub(crate) enum InventoryOwner {
    Npc(InstanceId),
    Chest(Entity),
}

/// Which panels the inventory UI currently shows and what confirm does.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum InventoryMode {
    #[default]
    Player,
    Trade,
    Loot,
}

/// Request to open the trade panels for the player and the trader npc.
#[derive(Message)]
pub(crate) struct OpenTrade {
    pub(crate) player: InstanceId,
    pub(crate) trader: InstanceId,
}

/// Request to open the loot panels for the player and a chest entity.
#[derive(Message)]
pub(crate) struct OpenLoot {
    pub(crate) player: InstanceId,
    pub(crate) chest: Entity,
}

/// State of the inventory UI: closed by default, items snapshot on open.
#[derive(Resource, Default)]
pub(crate) struct InventoryUiState {
    pub(crate) open: bool,
    pub(crate) mode: InventoryMode,
    /// Windows by index: [left, player], the left one only exists outside the
    /// player-only mode.
    pub(crate) windows: [Option<InventoryWindowState>; 2],
    /// Index of the window whose grid receives the navigation input.
    pub(crate) active_window: usize,
    /// Gold item instance used for payments, resolved when a trade opens.
    pub(crate) gold_item: Option<InstanceId>,
}

impl InventoryUiState {
    pub(crate) fn is_open(&self) -> bool {
        self.open
    }

    pub(crate) fn close(&mut self) {
        self.open = false;
        self.mode = InventoryMode::default();
        self.windows = Default::default();
        self.active_window = 0;
        self.gold_item = None;
    }

    pub(crate) fn is_trade(&self) -> bool {
        self.mode == InventoryMode::Trade
    }
}

/// Window index of the trader panel (left side).
pub(crate) const TRADER_WINDOW: usize = 0;
/// Window index of the player panel (right side).
pub(crate) const PLAYER_WINDOW: usize = 1;

/// `TRADE_VALUE_MULTIPLIER` from the Gothic 2 scripts: the share of an item's
/// value a trader pays when buying it from the player.
const TRADE_VALUE_MULTIPLIER: f32 = 0.15;
