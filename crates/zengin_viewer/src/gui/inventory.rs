//! Placeholder inventory UI: slot grid panels like the original Gothic
//! inventory (header bar with title and gold, item slots, info panel at the
//! bottom left). Items are shown as their (truncated) name text instead of 3D
//! models; visuals follow the original `INV_*` menu textures.
//!
//! The UI is built from reusable "inventory window" panels keyed by a window
//! index: today only the player inventory exists, but a trade scene later adds
//! a second panel (trader side) reusing the same spawn and sync code.

use bevy::prelude::*;
use leafwing_input_manager::prelude::ActionState;

use crate::gameplay::objects::PlayerCharacter;
use crate::gui::game_log::GameLog;
use crate::input::actions::{GuiAction, GuiInputMarker, SharedAction, SharedInputMarker};
use crate::zengin::dialog::DialogUiState;
use crate::zengin::{GameNpc, InstanceId, ScriptResource, WeaponKind};

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
    const TEXT_SHADOW: TextShadow = TextShadow {
        offset: Vec2::new(2.0, 2.0),
        color: Color::BLACK,
    };
}

/// Path of an original inventory menu texture, loaded through the ZenGin
/// compiled texture loader.
fn inv_texture(name: &str) -> String {
    format!("zengin://_WORK/DATA/TEXTURES/_COMPILED/{name}-C.TEX")
}

/// One entry shown in a grid: an inventory stack with its display name.
struct InventoryEntry {
    instance_id: InstanceId,
    count: i32,
    name: String,
    /// Whether the item is a gold stack, based on the script symbol name so
    /// localized display names do not break the gold sum.
    is_gold: bool,
}

/// Display name of an item instance symbol: strips the script item class
/// prefix (`itar_`/`itri_`/`itam_`/`itmi_`/...) and `addon` markers, so
/// `itri_addon_health_01` becomes `health_01`.
pub(crate) fn display_item_name(symbol_name: &str) -> String {
    let lowered = symbol_name.to_lowercase();
    let stripped =
        if lowered.len() >= 5 && lowered.starts_with("it") && lowered.as_bytes()[4] == b'_' {
            &lowered[5..]
        } else {
            &lowered[..]
        };
    stripped
        .replace("addon_", "")
        .replace("_addon", "")
        .replace("addon", "")
}

/// One window's state: open panels snapshot their items on open.
struct InventoryWindowState {
    /// Header title, e.g. a category name or the trader name.
    title: String,
    selected: usize,
    /// Currently equipped weapons of this window's npc, shown with a red slot.
    equipped_melee: Option<InstanceId>,
    equipped_ranged: Option<InstanceId>,
    items: Vec<InventoryEntry>,
}

impl InventoryWindowState {
    /// Gold shown in the header, sum of all gold item stacks.
    fn gold(&self) -> i32 {
        self.items
            .iter()
            .filter(|entry| entry.is_gold)
            .map(|entry| entry.count)
            .sum()
    }
}

/// State of the inventory UI: closed by default, items snapshot on open.
/// `windows[0]` is the player inventory, further windows are trade partners.
#[derive(Resource, Default)]
pub struct InventoryUiState {
    open: bool,
    windows: Vec<InventoryWindowState>,
}

impl InventoryUiState {
    pub(crate) fn is_open(&self) -> bool {
        self.open
    }

    /// Closes the inventory UI.
    pub(crate) fn close(&mut self) {
        self.open = false;
    }

    /// Window whose grid receives the navigation input.
    fn active_window(&mut self) -> Option<&mut InventoryWindowState> {
        self.windows.first_mut()
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
#[allow(dead_code)] // window index will be used for trade panels
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

pub struct InventoryUiPlugin;

impl Plugin for InventoryUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InventoryUiState>();
        app.add_systems(Startup, spawn_inventory_ui);
        app.add_systems(
            Update,
            (toggle_inventory, handle_inventory_input, sync_inventory_ui).chain(),
        );
    }
}

/// Textures shared by all inventory windows
#[derive(Resource)]
struct InventoryTextures {
    slot: Handle<Image>,
    focus: Handle<Image>,
}

/// Spawns the whole inventory UI once at startup, hidden until opened.
/// One window panel for the player inventory; trade adds more panels here.
fn spawn_inventory_ui(mut commands: Commands, asset_server: Res<AssetServer>) {
    let textures = InventoryTextures {
        slot: asset_server.load(inv_texture("INV_SLOT")),
        focus: asset_server.load(inv_texture("INV_SLOT_FOCUS")),
    };

    let root = commands
        .spawn((
            InventoryContainer::Root,
            Visibility::Hidden,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
        ))
        .id();

    // Player inventory window, top right like the original inventory
    spawn_inventory_window(
        &mut commands,
        root,
        0,
        "Inventory".to_string(),
        UiRect {
            left: Val::Auto,
            right: Val::Px(24.0),
            top: Val::Px(24.0),
            bottom: Val::Auto,
        },
        &textures,
    );

    // Info panel at the bottom left, like the original item description box
    commands
        .spawn((
            ChildOf(root),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(24.0),
                bottom: Val::Px(24.0),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                width: Val::Px(560.0),
                padding: UiRect::all(Val::Px(16.0)),
                ..default()
            },
            BackgroundColor(InventoryStyle::PANEL_BACKGROUND),
        ))
        .with_children(|info_panel| {
            info_panel.spawn((
                InventoryInfoText,
                Text::new(String::new()),
                TextFont {
                    font_size: InventoryStyle::INFO_FONT_SIZE.into(),
                    ..default()
                },
                TextColor(InventoryStyle::TEXT_COLOR),
                TextLayout::default().with_justify(Justify::Left),
                InventoryStyle::TEXT_SHADOW,
            ));
        });

    commands.insert_resource(textures);
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
                left: anchors.left.clone(),
                right: anchors.right.clone(),
                top: anchors.top.clone(),
                bottom: anchors.bottom.clone(),
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
            Text::new(text),
            TextFont {
                font_size: InventoryStyle::HEADER_FONT_SIZE.into(),
                ..default()
            },
            TextColor(InventoryStyle::TEXT_COLOR),
            TextLayout::default().with_justify(Justify::Center),
            InventoryStyle::TEXT_SHADOW,
        ));
    }

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
            Text::new(String::new()),
            TextFont {
                font_size: InventoryStyle::SLOT_FONT_SIZE.into(),
                ..default()
            },
            TextColor(InventoryStyle::TEXT_COLOR),
            TextLayout::default().with_justify(Justify::Center),
            InventoryStyle::TEXT_SHADOW,
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
            Text::new(String::new()),
            TextFont {
                font_size: InventoryStyle::COUNT_FONT_SIZE.into(),
                ..default()
            },
            TextColor(InventoryStyle::COUNT_TEXT_COLOR),
            InventoryStyle::TEXT_SHADOW,
        ));
    }

    panel
}

/// Opens the inventory on the gameplay key and closes it again (toggle).
/// The dialog UI takes precedence: no inventory while a dialog is open.
fn toggle_inventory(
    action_state: Single<&ActionState<SharedAction>, With<SharedInputMarker>>,
    dialog_ui: Res<DialogUiState>,
    mut state: ResMut<InventoryUiState>,
    gothic_script: Res<ScriptResource>,
    player_npc: Query<&GameNpc, With<PlayerCharacter>>,
) {
    if dialog_ui.is_open() || !action_state.just_pressed(&SharedAction::Inventory) {
        return;
    }
    if state.open {
        state.open = false;
        return;
    }

    let Ok(player_npc) = player_npc.single() else {
        return;
    };
    let player_id = player_npc.instance_id();
    let mut script = gothic_script.lock();
    let items = script.inventory(player_id);
    let mut items: Vec<_> = items
        .into_iter()
        .map(|(instance_id, count)| {
            let symbol_name = script
                .get_name_by_id(instance_id.value())
                .unwrap_or_default();
            let is_gold = symbol_name.to_lowercase().contains("gold");
            let name = script
                .item_display_name(instance_id)
                .unwrap_or_else(|| display_item_name(&symbol_name));
            InventoryEntry {
                instance_id,
                count,
                name,
                is_gold,
            }
        })
        .collect();
    // Deterministic grid order (the script inventory is a hash map)
    items.sort_unstable_by_key(|entry| entry.instance_id.value());
    let (equipped_melee, equipped_ranged) = script
        .npc_instances()
        .get(&player_id)
        .map(|npc| (npc.equipped_melee, npc.equipped_ranged))
        .unwrap_or_default();
    state.windows = vec![InventoryWindowState {
        title: "Inventory".to_string(),
        selected: 0,
        equipped_melee,
        equipped_ranged,
        items,
    }];
    state.open = true;
    info!(
        "Inventory opened with {} item stack(s)",
        state.windows[0].items.len()
    );
}

/// Grid navigation and closing while the inventory is open. Input goes to the
/// active window; with a single window that is always the player inventory.
/// Confirm equips the selected item as the player's drawn weapon.
fn handle_inventory_input(
    action_state: Single<&ActionState<GuiAction>, With<GuiInputMarker>>,
    mut state: ResMut<InventoryUiState>,
    mut game_log: ResMut<GameLog>,
    gothic_script: Res<ScriptResource>,
    player_npc: Query<&GameNpc, With<PlayerCharacter>>,
) {
    if !state.open {
        return;
    }
    if action_state.just_pressed(&GuiAction::CancelAction) {
        state.open = false;
        return;
    }

    let Some(window) = state.active_window() else {
        return;
    };
    let slot_count = window
        .items
        .len()
        .min(InventoryStyle::COLUMNS * InventoryStyle::ROWS);
    if slot_count == 0 {
        return;
    }
    let selected = &mut window.selected;

    let columns = InventoryStyle::COLUMNS;
    if action_state.just_pressed(&GuiAction::Left) {
        if *selected % columns > 0 {
            *selected -= 1;
        }
    }
    if action_state.just_pressed(&GuiAction::Right) {
        if *selected + 1 < slot_count && *selected % columns < columns - 1 {
            *selected += 1;
        }
    }
    if action_state.just_pressed(&GuiAction::Up) {
        if *selected >= columns {
            *selected -= columns;
        }
    }
    if action_state.just_pressed(&GuiAction::Down) {
        if *selected + columns < slot_count {
            *selected += columns;
        }
    }
    if action_state.just_pressed(&GuiAction::Confirm)
        && let Some(entry) = window.items.get(*selected)
    {
        let (equipped_melee, equipped_ranged) =
            equip_selected_item(entry, &mut game_log, &gothic_script, player_npc);
        window.equipped_melee = equipped_melee;
        window.equipped_ranged = equipped_ranged;
    }
}

/// Equips the selected item as the player's weapon of its kind on confirm, a
/// second confirm unequips it again. A melee and a ranged weapon can be
/// equipped at the same time, equipping again replaces the same slot. Only
/// weapon items (`itmw_*`/`itrw_*` symbols) are accepted, everything else is
/// reported in the game log instead. Equipping sheathes the weapon, drawing is
/// done by the dedicated weapon inputs. Returns the items equipped afterwards.
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
fn sync_inventory_ui(
    state: Res<InventoryUiState>,
    textures: Res<InventoryTextures>,
    mut containers: Query<(&InventoryContainer, &mut Visibility)>,
    mut slots: Query<
        (&InventorySlot, &mut Node, &mut ImageNode, &Children),
        Without<InventorySlotEquippedOverlay>,
    >,
    mut equipped_overlays: Query<&mut Node, With<InventorySlotEquippedOverlay>>,
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
        &mut Text,
        (
            With<InventoryInfoText>,
            Without<InventorySlotTextOrCount>,
            Without<InventoryWindowTitleText>,
            Without<InventoryWindowGoldText>,
        ),
    >,
) {
    for (_, mut visibility) in containers.iter_mut() {
        let desired = if state.open {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if *visibility != desired {
            *visibility = desired;
        }
    }
    if !state.open {
        return;
    }

    for (window_index, window) in state.windows.iter().enumerate() {
        if let Some((_, mut text)) = window_title
            .iter_mut()
            .find(|(marker, _)| marker.0 == window_index)
        {
            if text.0 != window.title {
                text.0 = window.title.clone();
            }
        }
        if let Some((_, mut text)) = window_gold
            .iter_mut()
            .find(|(marker, _)| marker.0 == window_index)
        {
            let gold = format!("Gold: {}", window.gold());
            if text.0 != gold {
                text.0 = gold;
            }
        }

        let slot_count = window
            .items
            .len()
            .min(InventoryStyle::COLUMNS * InventoryStyle::ROWS);
        for (slot, mut node, mut image, children) in slots.iter_mut() {
            if slot.window != window_index {
                continue;
            }
            let used = slot.index < slot_count;
            node.display = if used { Display::Flex } else { Display::None };
            if !used {
                continue;
            }
            let entry = &window.items[slot.index];
            let is_equipped = window.equipped_melee == Some(entry.instance_id)
                || window.equipped_ranged == Some(entry.instance_id);
            let texture = if slot.index == window.selected {
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
                    let desired = if is_equipped {
                        Display::Flex
                    } else {
                        Display::None
                    };
                    if overlay.display != desired {
                        overlay.display = desired;
                    }
                    continue;
                }
                if let Ok((mut text, kind)) = slot_texts.get_mut(child) {
                    let desired = match kind {
                        InventorySlotTextOrCount::Name => display_text.clone(),
                        InventorySlotTextOrCount::Count => count_text.clone(),
                    };
                    if text.0 != desired {
                        text.0 = desired;
                    }
                }
            }
        }
    }

    if let Ok(mut text) = info_text.single_mut() {
        let info = state
            .windows
            .first()
            .and_then(|window| window.items.get(window.selected))
            .map(|entry| format!("{}\ncount: {}", entry.name, entry.count))
            .unwrap_or_default();
        if text.0 != info {
            text.0 = info;
        }
    }
}
