use bevy::prelude::*;

use crate::ui_kit::{UiText, spawn_gui_root};
use crate::zengin::get_full_texture_path;

use super::state::{PLAYER_WINDOW, TRADER_WINDOW};
use super::style::InventoryStyle;

/// Markers for the persistent inventory UI entities, toggled via [`Visibility`]
#[derive(Component)]
pub(super) enum InventoryContainer {
    Root,
}

/// Root node of one inventory window panel, holds its index into
/// [`super::state::InventoryUiState::windows`]
#[derive(Component)]
pub(super) struct InventoryWindowPanel(pub(super) usize);

/// Single grid slot: window index + slot index in that window's items list
#[derive(Component)]
pub(super) struct InventorySlot {
    pub(super) window: usize,
    pub(super) index: usize,
}

/// Name text inside a grid slot
#[derive(Component)]
pub(super) enum InventorySlotTextOrCount {
    Name,
    Count,
}

/// Header title text of one window panel
#[derive(Component)]
pub(super) struct InventoryWindowTitleText(pub(super) usize);

/// Header gold text of one window panel
#[derive(Component)]
pub(super) struct InventoryWindowGoldText(pub(super) usize);

/// Red translucent fill over an equipped item's slot
#[derive(Component)]
pub(super) struct InventorySlotEquippedOverlay;

/// Item info text in the bottom left panel
#[derive(Component)]
pub(super) struct InventoryInfoText;

/// Right-aligned value column of the item info panel
#[derive(Component)]
pub(super) struct InventoryInfoValues;

/// Textures shared by all inventory windows
#[derive(Resource)]
pub(super) struct InventoryTextures {
    pub(super) slot: Handle<Image>,
    pub(super) focus: Handle<Image>,
}

/// Spawns the whole inventory UI once at startup, hidden until opened.
pub(super) fn spawn_inventory_ui(mut commands: Commands, asset_server: Res<AssetServer>) {
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
