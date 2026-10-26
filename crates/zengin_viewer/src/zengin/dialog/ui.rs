use super::state::{DialogLineView, DialogOptionView, DialogUiState};
use bevy::prelude::*;

use crate::ui_kit::{
    UiText, follow_selected_row, set_display, set_font_size, set_text, set_text_color,
    set_visibility, spawn_gui_root,
};

/// Visual style of the dialog UI, single place to tweak the look
pub(super) struct DialogStyle;

impl DialogStyle {
    pub(super) const FONT_SIZE: f32 = 22.0;
    pub(super) const SELECTED_FONT_SIZE: f32 = 24.0;
    pub(super) const ROW_HEIGHT: f32 = 26.0;
    pub(super) const OPTIONS_WIDTH: f32 = 800.0;
    pub(super) const OPTIONS_MAX_HEIGHT: f32 = 280.0;
    pub(super) const LINES_WIDTH: f32 = 640.0;
    pub(super) const PANEL_BACKGROUND: Color = Color::srgba(0.0, 0.0, 0.0, 0.75);
    pub(super) const TEXT_COLOR_NORMAL: Color = Color::srgb(0.6, 0.6, 0.6);
    pub(super) const TEXT_COLOR_SELECTED: Color = Color::srgb(0.95, 0.95, 0.95);
    pub(super) const LINE_TEXT_COLOR: Color = Color::srgb(0.9, 0.75, 0.35);
}

#[derive(Component)]
pub(super) enum DialogContainer {
    Root,
    OptionsPanel,
    LinesPanel,
}

/// Scrollable container of the option rows
#[derive(Component)]
pub(super) struct DialogRowsContainer;

/// Single row in the options panel: an option with its root index
#[derive(Component)]
pub(super) struct DialogRow(usize);

/// Text showing the symbol name of the npc the dialog is open with
#[derive(Component)]
pub(super) struct DialogNpcNameText;

/// Text showing the currently displayed dialog line
#[derive(Component)]
pub(super) struct DialogLineText;

/// Query data for the npc name text, disjoint from all other dialog texts
pub(super) type NpcNameTextFilter = (
    With<DialogNpcNameText>,
    Without<DialogRow>,
    Without<DialogLineText>,
);

/// Query data for the dialog line text, disjoint from all other dialog texts
pub(super) type LineTextFilter = (
    With<DialogLineText>,
    Without<DialogRow>,
    Without<DialogNpcNameText>,
);

/// Replaces bevy's built-in default font (`FiraMono` subset, ASCII-only) with Fira Sans,
/// which has Latin Extended-A coverage, so Polish dialog texts render properly.
/// Applies globally because every `TextFont { ..default() }` resolves to the default font asset id.
pub(super) fn override_default_font(mut fonts: ResMut<Assets<Font>>) {
    const FONT_DATA: &[u8] = include_bytes!("../../../assets/fonts/FiraSans-Regular.ttf");
    fonts
        .insert(
            bevy::asset::AssetId::default(),
            Font::from_bytes(FONT_DATA.to_vec()),
        )
        .unwrap();
}

/// Option rows are spawned lazily on demand and never removed, unused ones get `Display::None`.
pub(super) fn spawn_dialog_ui(mut commands: Commands) {
    let root = spawn_gui_root(&mut commands, DialogContainer::Root);
    spawn_options_panel(&mut commands, root);
    spawn_lines_panel(&mut commands, root);
}

fn spawn_options_panel(commands: &mut Commands, root: Entity) {
    let options_panel = commands
        .spawn((
            ChildOf(root),
            DialogContainer::OptionsPanel,
            Visibility::Hidden,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                bottom: Val::Px(24.0),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                ..default()
            },
        ))
        .id();

    commands.spawn((
        ChildOf(options_panel),
        DialogRowsContainer,
        ScrollPosition::default(),
        BackgroundColor(DialogStyle::PANEL_BACKGROUND),
        Node {
            padding: UiRect {
                left: Val::Px(14.0),
                right: Val::Px(14.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
            },
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            width: Val::Px(DialogStyle::OPTIONS_WIDTH),
            max_height: Val::Px(DialogStyle::OPTIONS_MAX_HEIGHT),
            overflow: Overflow::scroll_y(),
            ..default()
        },
    ));
}

fn spawn_lines_panel(commands: &mut Commands, root: Entity) {
    commands
        .spawn((
            ChildOf(root),
            DialogContainer::LinesPanel,
            Visibility::Hidden,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                top: Val::Px(24.0),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                ..default()
            },
        ))
        .with_children(|lines_panel| {
            lines_panel
                .spawn((
                    BackgroundColor(DialogStyle::PANEL_BACKGROUND),
                    Node {
                        display: Display::Flex,
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        width: Val::Px(DialogStyle::LINES_WIDTH),
                        padding: UiRect::all(Val::Px(8.0)),
                        ..default()
                    },
                ))
                .with_children(|lines_box| {
                    lines_box.spawn((
                        DialogNpcNameText,
                        UiText::new(
                            String::new(),
                            DialogStyle::FONT_SIZE,
                            DialogStyle::LINE_TEXT_COLOR,
                        ),
                    ));
                    lines_box.spawn((
                        DialogLineText,
                        Node {
                            display: Display::Flex,
                            width: Val::Percent(100.0),
                            ..default()
                        },
                        UiText::new(
                            String::new(),
                            DialogStyle::FONT_SIZE,
                            DialogStyle::LINE_TEXT_COLOR,
                        )
                        .justify(Justify::Center),
                    ));
                });
        });
}

pub(super) fn sync_dialog_ui(
    mut commands: Commands,
    mut dialog_ui: ResMut<DialogUiState>,
    inventory_ui: Res<crate::gui::InventoryUiState>,
    mut containers: Query<(&DialogContainer, &mut Visibility)>,
    mut rows: Query<(
        Entity,
        &DialogRow,
        &mut Node,
        &mut Text,
        &mut TextColor,
        &mut TextFont,
    )>,
    mut rows_container: Query<
        (Entity, &mut ScrollPosition, &ComputedNode),
        With<DialogRowsContainer>,
    >,
    mut npc_name_text: Query<&mut Text, NpcNameTextFilter>,
    mut line_text: Query<&mut Text, LineTextFilter>,
) {
    let trade_open = inventory_ui.is_trade();
    if !dialog_ui.is_open() {
        sync_container_visibility(&mut containers, &dialog_ui, trade_open);
        return;
    }

    sync_container_visibility(&mut containers, &dialog_ui, trade_open);
    if dialog_ui.rows_dirty {
        sync_option_rows(
            &mut commands,
            &mut dialog_ui,
            &mut rows,
            &mut rows_container,
        );
    }
    if dialog_ui.is_showing_options() {
        sync_row_highlight(&dialog_ui, &mut rows);
        sync_scroll_follow(&dialog_ui, &mut rows_container);
    } else {
        sync_line_texts(&dialog_ui, &mut npc_name_text, &mut line_text);
    }
}

fn sync_container_visibility(
    containers: &mut Query<(&DialogContainer, &mut Visibility)>,
    dialog_ui: &DialogUiState,
    trade_open: bool,
) {
    let open = dialog_ui.is_open();
    for (container, mut visibility) in containers.iter_mut() {
        let visible = match container {
            DialogContainer::Root => open,
            DialogContainer::OptionsPanel => open && dialog_ui.is_showing_options() && !trade_open,
            DialogContainer::LinesPanel => open && dialog_ui.is_playing_lines(),
        };
        set_visibility(&mut visibility, visible);
    }
}

fn sync_option_rows(
    commands: &mut Commands,
    dialog_ui: &mut DialogUiState,
    rows: &mut Query<(
        Entity,
        &DialogRow,
        &mut Node,
        &mut Text,
        &mut TextColor,
        &mut TextFont,
    )>,
    rows_container: &mut Query<
        (Entity, &mut ScrollPosition, &ComputedNode),
        With<DialogRowsContainer>,
    >,
) {
    dialog_ui.rows_dirty = false;

    // Rows are only ever appended, so their index matches their position
    let mut indexed_rows: Vec<(usize, Entity)> = rows
        .iter()
        .map(|(entity, row, _, _, _, _)| (row.0, entity))
        .collect();
    indexed_rows.sort_unstable_by_key(|(index, _)| *index);

    let Ok((rows_container_entity, mut scroll_position, _)) = rows_container.single_mut() else {
        return;
    };

    let active_choices = dialog_ui.active_choices();

    for (index, choice) in active_choices.iter().enumerate() {
        let display_text = dialog_row_display_text(choice);
        match indexed_rows.get(index) {
            Some((_, entity)) => {
                let Ok((_, _, mut node, mut text, _, _)) = rows.get_mut(*entity) else {
                    continue;
                };
                set_text(&mut text, &display_text);
                set_display(&mut node, true);
            }
            None => spawn_option_row(
                commands,
                rows_container_entity,
                index,
                display_text,
                index == dialog_ui.selected_index,
            ),
        }
    }

    // Hide surplus rows left over from previously opened dialogs
    for (_, entity) in indexed_rows.iter().skip(active_choices.len()) {
        let Ok((_, _, mut node, _, _, _)) = rows.get_mut(*entity) else {
            continue;
        };
        set_display(&mut node, false);
    }

    scroll_position.0 = Vec2::ZERO;
}

fn sync_row_highlight(
    dialog_ui: &DialogUiState,
    rows: &mut Query<(
        Entity,
        &DialogRow,
        &mut Node,
        &mut Text,
        &mut TextColor,
        &mut TextFont,
    )>,
) {
    for (_, row, _, _, text_color, text_font) in rows.iter_mut() {
        let selected = row.0 == dialog_ui.selected_index;
        apply_row_selection(text_color, text_font, selected);
    }
}

fn apply_row_selection(
    mut text_color: Mut<TextColor>,
    mut text_font: Mut<TextFont>,
    selected: bool,
) {
    let color = if selected {
        DialogStyle::TEXT_COLOR_SELECTED
    } else {
        DialogStyle::TEXT_COLOR_NORMAL
    };
    set_text_color(&mut text_color, color);
    let font_size = if selected {
        DialogStyle::SELECTED_FONT_SIZE
    } else {
        DialogStyle::FONT_SIZE
    };
    set_font_size(&mut text_font, font_size);
}

fn sync_scroll_follow(
    dialog_ui: &DialogUiState,
    rows_container: &mut Query<
        (Entity, &mut ScrollPosition, &ComputedNode),
        With<DialogRowsContainer>,
    >,
) {
    if let Ok((_, mut scroll_position, computed)) = rows_container.single_mut() {
        follow_selected_row(
            &mut scroll_position,
            computed,
            dialog_ui.selected_index,
            DialogStyle::ROW_HEIGHT,
        );
    }
}

fn sync_line_texts(
    dialog_ui: &DialogUiState,
    npc_name_text: &mut Query<&mut Text, NpcNameTextFilter>,
    line_text: &mut Query<&mut Text, LineTextFilter>,
) {
    if let Ok(mut text) = npc_name_text.single_mut() {
        set_text(&mut text, &dialog_ui.npc_name);
    }
    if let Some(view) = dialog_ui.current_line()
        && let Ok(mut text) = line_text.single_mut()
    {
        set_text(&mut text, &dialog_line_display_text(view));
    }
}

fn spawn_option_row(
    commands: &mut Commands,
    parent: Entity,
    index: usize,
    display_text: String,
    selected: bool,
) {
    commands.spawn((
        ChildOf(parent),
        DialogRow(index),
        Node {
            display: Display::Flex,
            height: Val::Px(DialogStyle::ROW_HEIGHT),
            overflow: Overflow::clip(),
            ..default()
        },
        UiText::new(
            display_text,
            if selected {
                DialogStyle::SELECTED_FONT_SIZE
            } else {
                DialogStyle::FONT_SIZE
            },
            if selected {
                DialogStyle::TEXT_COLOR_SELECTED
            } else {
                DialogStyle::TEXT_COLOR_NORMAL
            },
        ),
    ));
}

/// Option description, fall back to the function id when the description is empty
fn dialog_row_display_text(root: &DialogOptionView) -> String {
    if root.description.is_empty() {
        format!("<function {}>", root.sequence_function_id)
    } else {
        root.description.clone()
    }
}

/// Real text when available, otherwise everything known about the line for debugging
fn dialog_line_display_text(view: &DialogLineView) -> String {
    if let Some(text) = &view.text {
        return text.clone();
    }
    format!(
        "MISSING TEXT npc_a({}) npc_b({}) line_id({}) block(unresolvable)",
        view.npc_a, view.npc_b, view.line_id
    )
}
