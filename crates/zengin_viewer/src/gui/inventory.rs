//! Placeholder inventory UI: item slot grids and item info panels. Items are
//! shown as their (truncated) name text instead of 3D models.
//!
// TODO replace item text with 3d models
//!
//! Two reusable "inventory window" panels exist: the trader side on the left
//! and the player side on the right. A normal inventory shows only the player
//! panel, a trade shows both and enables buying/selling.

use bevy::log::{info, warn};
use bevy::prelude::*;
use leafwing_input_manager::prelude::ActionState;

use crate::gameplay::objects::PlayerCharacter;
use crate::gui::GameLog;
use crate::gui::JournalUiState;
use crate::input::{GuiAction, GuiInputMarker, SharedAction, SharedInputMarker};
use crate::ui_kit::{UiText, set_display, set_text, set_visibility, spawn_gui_root};
use crate::zengin::DialogUiState;
use crate::zengin::ScriptManager;
use crate::zengin::get_full_texture_path;
use crate::zengin::{GameChest, GameNpc, InstanceId, ItemDisplayData, ScriptResource, WeaponKind};

/// Window index of the trader panel (left side).
const TRADER_WINDOW: usize = 0;
/// Window index of the player panel (right side).
const PLAYER_WINDOW: usize = 1;

/// `TRADE_VALUE_MULTIPLIER` from the Gothic 2 scripts: the share of an item's
/// value a trader pays when buying it from the player.
const TRADE_VALUE_MULTIPLIER: f32 = 0.15;

pub struct InventoryUiPlugin;

impl Plugin for InventoryUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InventoryUiState>();
        app.add_message::<OpenTrade>();
        app.add_message::<OpenLoot>();
        app.add_systems(Startup, spawn_inventory_ui);
        app.add_systems(
            Update,
            (
                open_trade,
                open_loot,
                toggle_inventory,
                handle_inventory_input,
                close_orphaned_trade,
                sync_inventory_ui,
            )
                .chain(),
        );
    }
}

/// Visual style of the inventory UI, single place to tweak the look
struct InventoryStyle;

impl InventoryStyle {
    const COLUMNS: usize = 5;
    const ROWS: usize = 8;
    const SLOT_SIZE: f32 = 90.0;
    const SLOT_GAP: f32 = 4.0;
    const PANEL_PADDING: f32 = 12.0;
    const SLOT_FONT_SIZE: f32 = 18.0;
    const COUNT_FONT_SIZE: f32 = 15.0;
    const HEADER_FONT_SIZE: f32 = 24.0;
    const INFO_FONT_SIZE: f32 = 24.0;
    const SLOT_NAME_CHARS: usize = 7;
    const PANEL_BACKGROUND: Color = Color::srgba(0.06, 0.05, 0.04, 0.85);
    /// Translucent red fill drawn over equipped item slots.
    const EQUIPPED_OVERLAY: Color = Color::srgba(0.7, 0.05, 0.05, 0.1);
    const TEXT_COLOR: Color = Color::srgb(0.9, 0.8, 0.6);
    const COUNT_TEXT_COLOR: Color = Color::srgb(0.95, 0.95, 0.9);
}

/// One entry shown in a grid: an inventory stack with its display name.
struct InventoryEntry {
    instance_id: InstanceId,
    count: i32,
    name: String,
    /// Stats shown in the info panel, `None` when the item has no script data.
    stats: Option<ItemDisplayData>,
    /// Whether the item is a gold stack, based on the script symbol name so
    /// localized display names do not break the gold sum.
    is_gold: bool,
    /// Sort group: melee weapon, ranged weapon, everything else.
    sort_group: u8,
    /// Base value (`C_Item.value`), the buy price.
    value: i32,
}

/// One window's state: open panels snapshot their items on open.
struct InventoryWindowState {
    /// Inventory source behind this window.
    owner: InventoryOwner,
    /// Header title, e.g. a category name or the trader name.
    title: String,
    selected: usize,
    /// Currently equipped weapons of this window's npc, shown with a red slot.
    equipped_melee: Option<InstanceId>,
    equipped_ranged: Option<InstanceId>,
    /// Gold stacks are not always shown in the grid, the header always needs them.
    gold_count: i32,
    items: Vec<InventoryEntry>,
}

/// Source of the items shown by one inventory window.
#[derive(Clone, Copy)]
enum InventoryOwner {
    Npc(InstanceId),
    Chest(Entity),
}

/// Which panels the inventory UI currently shows and what confirm does.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum InventoryMode {
    #[default]
    Player,
    Trade,
    Loot,
}

impl InventoryEntry {
    /// Price the player pays for this item.
    fn buy_price(&self) -> i32 {
        self.value.max(0)
    }

    /// Price the player receives for this item.
    fn sell_price(&self) -> i32 {
        (self.value.max(0) as f32 * TRADE_VALUE_MULTIPLIER).ceil() as i32
    }
}

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
fn npc_inventory_window(
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

fn chest_inventory_window(
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

/// Request to open the trade panels for the player and the trader npc.
#[derive(Message)]
pub struct OpenTrade {
    pub player: InstanceId,
    pub trader: InstanceId,
}

/// Request to open the loot panels for the player and a chest entity.
#[derive(Message)]
pub struct OpenLoot {
    pub player: InstanceId,
    pub chest: Entity,
}

/// State of the inventory UI: closed by default, items snapshot on open.
#[derive(Resource, Default)]
pub struct InventoryUiState {
    open: bool,
    mode: InventoryMode,
    /// Windows by index: [left, player], the left one only exists outside the
    /// player-only mode.
    windows: [Option<InventoryWindowState>; 2],
    /// Index of the window whose grid receives the navigation input.
    active_window: usize,
    /// Gold item instance used for payments, resolved when a trade opens.
    gold_item: Option<InstanceId>,
}

impl InventoryUiState {
    pub(crate) fn is_open(&self) -> bool {
        self.open
    }

    /// Closes the inventory UI.
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

/// Markers for the persistent inventory UI entities, toggled via [`Visibility`]
#[derive(Component)]
enum InventoryContainer {
    Root,
}

/// Root node of one inventory window panel, holds its index into
/// [`InventoryUiState::windows`]
#[derive(Component)]
struct InventoryWindowPanel(usize);

/// Single grid slot: window index + slot index in that window's items list
#[derive(Component)]
struct InventorySlot {
    window: usize,
    index: usize,
}

/// Name text inside a grid slot
#[derive(Component)]
enum InventorySlotTextOrCount {
    Name,
    Count,
}

/// Header title text of one window panel
#[derive(Component)]
struct InventoryWindowTitleText(usize);

/// Header gold text of one window panel
#[derive(Component)]
struct InventoryWindowGoldText(usize);

/// Red translucent fill over an equipped item's slot
#[derive(Component)]
struct InventorySlotEquippedOverlay;

/// Item info text in the bottom left panel
#[derive(Component)]
struct InventoryInfoText;

/// Right-aligned value column of the item info panel
#[derive(Component)]
struct InventoryInfoValues;

/// Textures shared by all inventory windows
#[derive(Resource)]
struct InventoryTextures {
    slot: Handle<Image>,
    focus: Handle<Image>,
}

/// Spawns the whole inventory UI once at startup, hidden until opened.
fn spawn_inventory_ui(mut commands: Commands, asset_server: Res<AssetServer>) {
    let textures = InventoryTextures {
        slot: asset_server.load(get_full_texture_path("INV_SLOT")),
        focus: asset_server.load(get_full_texture_path("INV_SLOT_FOCUS")),
    };

    let root = spawn_gui_root(&mut commands, InventoryContainer::Root);

    // Trader window on the left, only shown in trade mode
    spawn_inventory_window(
        &mut commands,
        root,
        TRADER_WINDOW,
        String::new(),
        UiRect {
            left: Val::Px(24.0),
            right: Val::Auto,
            top: Val::Px(24.0),
            bottom: Val::Auto,
        },
        &textures,
    );

    // Player inventory window on the right
    spawn_inventory_window(
        &mut commands,
        root,
        PLAYER_WINDOW,
        "Inventory".to_string(),
        UiRect {
            left: Val::Auto,
            right: Val::Px(24.0),
            top: Val::Px(24.0),
            bottom: Val::Auto,
        },
        &textures,
    );

    // Info panel at the bottom left, like the original item description box:
    // label column on the left, right-aligned values on the right. Both texts
    // have one line per row so the rows line up.
    commands
        .spawn((
            ChildOf(root),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(24.0),
                bottom: Val::Px(24.0),
                display: Display::Flex,
                flex_direction: FlexDirection::Row,
                width: Val::Px(560.0),
                padding: UiRect::all(Val::Px(16.0)),
                ..default()
            },
            BackgroundColor(InventoryStyle::PANEL_BACKGROUND),
        ))
        .with_children(|info_panel| {
            info_panel.spawn((
                InventoryInfoText,
                Node {
                    flex_grow: 1.0,
                    ..default()
                },
                UiText::new(
                    String::new(),
                    InventoryStyle::INFO_FONT_SIZE,
                    InventoryStyle::TEXT_COLOR,
                ),
            ));
            info_panel.spawn((
                InventoryInfoText,
                InventoryInfoValues,
                Node {
                    width: Val::Px(120.0),
                    ..default()
                },
                UiText::new(
                    String::new(),
                    InventoryStyle::INFO_FONT_SIZE,
                    InventoryStyle::TEXT_COLOR,
                )
                .justify(Justify::Right),
            ));
        });

    commands.insert_resource(textures);
}

/// Spawns one header box: a fixed size background node with the text as a
/// child, centered horizontally and vertically. The text must be a separate
/// child entity — `align_items` on the text's own node would only affect its
/// children, leaving the glyphs top-aligned in a fixed height box.
fn spawn_header_box(
    commands: &mut Commands,
    header: Entity,
    marker: impl Component,
    text: String,
    width: f32,
) {
    let box_node = commands
        .spawn((
            ChildOf(header),
            BackgroundColor(InventoryStyle::PANEL_BACKGROUND),
            Node {
                display: Display::Flex,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                width: Val::Px(width),
                height: Val::Px(InventoryStyle::SLOT_SIZE * 0.6),
                ..default()
            },
        ))
        .id();
    commands.spawn((
        ChildOf(box_node),
        marker,
        UiText::new(
            text,
            InventoryStyle::HEADER_FONT_SIZE,
            InventoryStyle::TEXT_COLOR,
        )
        .justify(Justify::Center),
    ));
}

/// Spawns one inventory window panel: header bar (title + gold) above a slot
/// grid. Returns the panel entity. `anchors` places the panel on screen; a
/// trade scene spawns a second panel anchored to the other screen side.
fn spawn_inventory_window(
    commands: &mut Commands,
    root: Entity,
    window: usize,
    title: String,
    anchors: UiRect,
    textures: &InventoryTextures,
) -> Entity {
    let grid_width = InventoryStyle::COLUMNS as f32 * InventoryStyle::SLOT_SIZE
        + (InventoryStyle::COLUMNS as f32 - 1.0) * InventoryStyle::SLOT_GAP
        + 2.0 * InventoryStyle::PANEL_PADDING;

    let panel = commands
        .spawn((
            ChildOf(root),
            InventoryWindowPanel(window),
            Node {
                position_type: PositionType::Absolute,
                left: anchors.left,
                right: anchors.right,
                top: anchors.top,
                bottom: anchors.bottom,
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                ..default()
            },
        ))
        .id();

    // Header bar: title box on the left, gold box on the right
    let header = commands
        .spawn((
            ChildOf(panel),
            Node {
                display: Display::Flex,
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(InventoryStyle::SLOT_GAP),
                margin: UiRect::bottom(Val::Px(InventoryStyle::SLOT_GAP)),
                ..default()
            },
        ))
        .id();
    spawn_header_box(
        commands,
        header,
        InventoryWindowTitleText(window),
        title,
        grid_width * 0.55,
    );
    spawn_header_box(
        commands,
        header,
        InventoryWindowGoldText(window),
        String::new(),
        grid_width * 0.45 - InventoryStyle::SLOT_GAP,
    );

    // Slot grid on the plain panel background, uniform across the window
    let grid = commands
        .spawn((
            ChildOf(panel),
            BackgroundColor(InventoryStyle::PANEL_BACKGROUND),
            Node {
                display: Display::Flex,
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                column_gap: Val::Px(InventoryStyle::SLOT_GAP),
                row_gap: Val::Px(InventoryStyle::SLOT_GAP),
                width: Val::Px(grid_width),
                padding: UiRect::all(Val::Px(InventoryStyle::PANEL_PADDING)),
                ..default()
            },
        ))
        .id();

    for index in 0..InventoryStyle::COLUMNS * InventoryStyle::ROWS {
        let slot = commands
            .spawn((
                ChildOf(grid),
                InventorySlot { window, index },
                ImageNode::new(textures.slot.clone()),
                Node {
                    display: Display::None,
                    width: Val::Px(InventoryStyle::SLOT_SIZE),
                    height: Val::Px(InventoryStyle::SLOT_SIZE),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..default()
                },
            ))
            .id();
        // Equipped marker: translucent red fill over the whole slot, like the
        // original trade inventory
        commands.spawn((
            ChildOf(slot),
            InventorySlotEquippedOverlay,
            BackgroundColor(InventoryStyle::EQUIPPED_OVERLAY),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                display: Display::None,
                ..default()
            },
        ));
        commands.spawn((
            ChildOf(slot),
            InventorySlotTextOrCount::Name,
            UiText::new(
                String::new(),
                InventoryStyle::SLOT_FONT_SIZE,
                InventoryStyle::TEXT_COLOR,
            )
            .justify(Justify::Center),
        ));
        // Stack count in the bottom right corner, like the original
        commands.spawn((
            ChildOf(slot),
            InventorySlotTextOrCount::Count,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(10.0),
                bottom: Val::Px(6.0),
                ..default()
            },
            UiText::new(
                String::new(),
                InventoryStyle::COUNT_FONT_SIZE,
                InventoryStyle::COUNT_TEXT_COLOR,
            ),
        ));
    }

    panel
}

/// Opens the trade panels: both inventories are snapshotted and the trader
/// side is focused first.
fn open_trade(
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

/// Opens the loot panels: the chest and player inventories are snapshotted and
/// the chest side is focused first.
fn open_loot(
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

/// Gold item used for payments: the known currency instance, falling back to
/// an inventory stack whose symbol name looks like gold.
fn resolve_gold_item(
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

/// The dialog UI takes precedence: no inventory while a dialog or the journal
/// is open.
fn toggle_inventory(
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
fn handle_inventory_input(
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
fn close_orphaned_trade(dialog_ui: Res<DialogUiState>, mut state: ResMut<InventoryUiState>) {
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

fn chest_remove_item(items: &mut Vec<(InstanceId, i32)>, item: InstanceId, count: i32) -> i32 {
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

fn chest_add_item(items: &mut Vec<(InstanceId, i32)>, item: InstanceId, count: i32) {
    if let Some(entry) = items.iter_mut().find(|(entry, _)| *entry == item) {
        entry.1 += count;
        return;
    }
    items.push((item, count));
}

fn clamp_selection(window: &mut Option<InventoryWindowState>, selected: usize) {
    let Some(window) = window else {
        return;
    };
    let max = window.items.len().saturating_sub(1);
    window.selected = selected.min(max);
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

/// Updates visibility, header texts, slot contents and the info panel from
/// the state.
#[allow(clippy::type_complexity, clippy::cognitive_complexity)]
fn sync_inventory_ui(
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
