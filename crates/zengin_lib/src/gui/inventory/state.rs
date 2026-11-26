use bevy::prelude::*;

use crate::{InstanceId, ItemDisplayData};

/// One entry shown in a grid: an inventory stack with its display name.
pub struct InventoryEntry {
    pub instance_id: InstanceId,
    pub count: i32,
    pub name: String,
    /// Stats shown in the info panel, `None` when the item has no script data.
    pub stats: Option<ItemDisplayData>,
    /// Whether the item is a gold stack, based on the script symbol name so
    /// localized display names do not break the gold sum.
    pub is_gold: bool,
    /// Sort group: melee weapon, ranged weapon, everything else.
    pub sort_group: u8,
    /// Base value (`C_Item.value`), the buy price.
    pub value: i32,
}

impl InventoryEntry {
    /// Price the player pays for this item.
    pub fn buy_price(&self) -> i32 {
        self.value.max(0)
    }

    /// Price the player receives for this item.
    pub fn sell_price(&self) -> i32 {
        (self.value.max(0) as f32 * TRADE_VALUE_MULTIPLIER).ceil() as i32
    }
}

/// One window's state: open panels snapshot their items on open.
pub struct InventoryWindowState {
    /// Inventory source behind this window.
    pub owner: InventoryOwner,
    /// Header title, e.g. a category name or the trader name.
    pub title: String,
    pub selected: usize,
    /// Currently equipped weapons of this window's npc, shown with a red slot.
    pub equipped_melee: Option<InstanceId>,
    pub equipped_ranged: Option<InstanceId>,
    /// Currently equipped armor of this window's npc.
    pub equipped_armor: Option<InstanceId>,
    /// Gold stacks are not always shown in the grid, the header always needs them.
    pub gold_count: i32,
    pub items: Vec<InventoryEntry>,
}

/// Source of the items shown by one inventory window.
#[derive(Clone, Copy)]
pub enum InventoryOwner {
    Npc(InstanceId),
    Chest,
}

/// Which panels the inventory UI currently shows and what confirm does.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum InventoryMode {
    #[default]
    Player,
    Trade,
    Loot,
    LockPick,
}

/// Request to open the trade panels for the player and the trader npc.
#[derive(Message)]
pub struct OpenTrade {
    pub player: InstanceId,
    pub trader: InstanceId,
}

/// Where the left loot panel takes items from.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LootSource {
    Chest(Entity),
    Corpse(Entity),
}

/// Request to open the loot panels for the player and a chest or corpse.
#[derive(Message)]
pub struct OpenLoot {
    pub player: InstanceId,
    pub source: LootSource,
}

/// Request to start the lockpick sequence on a chest.
#[derive(Message)]
pub struct OpenLockpick {
    pub player: InstanceId,
    pub chest: Entity,
}

/// The lockpick sequence completed, the chest is ready to be opened.
#[derive(Message)]
pub struct LockpickOpened {
    pub chest: Entity,
}

/// Lockpick sequence in progress.
pub struct LockpickSession {
    pub chest: Entity,
    /// `R`/`L` steps, other characters are dropped.
    pub sequence: Vec<char>,
    pub progress: usize,
}

impl LockpickSession {
    pub fn new(chest: Entity, pick_string: &str) -> Self {
        let sequence = pick_string
            .chars()
            .filter_map(|step| match step.to_ascii_uppercase() {
                'L' => Some('L'),
                'R' => Some('R'),
                _ => None,
            })
            .collect();
        Self {
            chest,
            sequence,
            progress: 0,
        }
    }
}

/// The loot panels closed, the player should leave the chest.
#[derive(Message)]
pub struct LootClosed {
    pub source: LootSource,
}

/// State of the inventory UI: closed by default, items snapshot on open.
#[derive(Resource, Default)]
pub struct InventoryUiState {
    pub open: bool,
    pub mode: InventoryMode,
    /// Windows by index: [left, player], the left one only exists outside the
    /// player-only mode.
    pub windows: [Option<InventoryWindowState>; 2],
    /// Index of the window whose grid receives the navigation input.
    pub active_window: usize,
    /// Gold item instance used for payments, resolved when a trade opens.
    pub gold_item: Option<InstanceId>,
    /// Chest or corpse of the loot session, consumed when the panels close.
    pub loot_source: Option<LootSource>,
    /// Active lockpick sequence, only set in [`InventoryMode::LockPick`].
    pub lockpick: Option<LockpickSession>,
}

impl InventoryUiState {
    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn close(&mut self) {
        self.open = false;
        self.mode = InventoryMode::default();
        self.windows = Default::default();
        self.active_window = 0;
        self.gold_item = None;
        self.lockpick = None;
        // `loot_source` stays until the close is consumed, so the interaction
        // side can leave the chest.
    }

    pub fn is_trade(&self) -> bool {
        self.mode == InventoryMode::Trade
    }
}

/// Window index of the trader panel (left side).
pub const TRADER_WINDOW: usize = 0;
/// Window index of the player panel (right side).
pub const PLAYER_WINDOW: usize = 1;

/// `TRADE_VALUE_MULTIPLIER` from the Gothic 2 scripts: the share of an item's
/// value a trader pays when buying it from the player.
const TRADE_VALUE_MULTIPLIER: f32 = 0.15;
