use bevy::color::Alpha;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::gameplay::game_time::GameTime;
use crate::zengin::gui::menu_screen::{item_rect, menu_area, menu_text};
use crate::zengin::{
    MENU_ITEM_FLAG_TXT_CENTER, MENU_SCRIPT_DIV, MenuDefinition, MenuItemDefinition,
    get_full_texture_path,
};
use dev_util::ui_kit::{UiText, follow_selected_row, set_text, set_text_color, set_visibility};

use super::data::JournalMenuData;
use super::spawn::{
    JournalContainerMarker, JournalDetailScrollMarker, JournalListMarker, JournalRootMarker,
    JournalTextMarker,
};
use super::state::{JournalCategory, JournalFocus, JournalUiState};
use super::style::JournalStyle;
use dev_util::perf_debug;

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn sync_journal_ui(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    data: Res<JournalMenuData>,
    windows: Query<&Window, With<PrimaryWindow>>,
    game_time: Res<GameTime>,
    mut state: ResMut<JournalUiState>,
    root: Single<(Entity, &mut Visibility), With<JournalRootMarker>>,
    mut containers: Query<(&JournalContainerMarker, &mut Visibility), Without<JournalRootMarker>>,
    mut texts: Query<(&JournalTextMarker, &mut Text, &mut TextColor)>,
    list: Option<
        Single<
            (&mut ScrollPosition, &ComputedNode),
            (With<JournalListMarker>, Without<JournalDetailScrollMarker>),
        >,
    >,
    detail_scroll: Option<
        Single<
            (&mut ScrollPosition, &ComputedNode),
            (With<JournalDetailScrollMarker>, Without<JournalListMarker>),
        >,
    >,
) {
    let _perf = perf_debug!();
    let (root_entity, mut root_visibility) = root.into_inner();
    let open = state.open;
    set_visibility(&mut root_visibility, open);
    if !open {
        return;
    }
    let Some(definition) = data.definition() else {
        return;
    };
    let (window_size, scale) =
        windows
            .iter()
            .next()
            .map_or((Vec2::new(640.0, 480.0), 1.0), |window| {
                (
                    Vec2::new(window.width(), window.height()),
                    window.scale_factor(),
                )
            });

    if state.take_rebuild(window_size, scale) {
        commands.entity(root_entity).despawn_children();
        rebuild_journal(
            &mut commands,
            &asset_server,
            definition,
            data.viewer_back_pic(),
            &mut state,
            window_size,
            scale,
            root_entity,
        );
    }

    for (container, mut visibility) in containers.iter_mut() {
        let visible = match container {
            JournalContainerMarker::Main => !state.detail_open,
            JournalContainerMarker::Detail => state.detail_open,
        };
        set_inherited_visibility(&mut visibility, visible);
    }

    if let Some(list) = list {
        let (mut scroll, computed) = list.into_inner();
        if state.focus == JournalFocus::Topics && !state.active.is_empty() {
            follow_selected_row(
                &mut scroll,
                computed,
                state.selected,
                JournalStyle::ROW_HEIGHT * scale,
            );
        }
    }

    let detail_text = if state.detail_dirty {
        state.detail_dirty = false;
        Some(state.detail_text())
    } else {
        None
    };
    let detail_footer = match detail_scroll {
        Some(scroll) if state.detail_open => Some(sync_detail_scroll(&mut state, scroll, scale)),
        _ => None,
    };
    let day = game_time.day.to_string();
    let time = time_text(&game_time);

    for (marker, mut text, mut color) in texts.iter_mut() {
        match marker {
            JournalTextMarker::Static => {}
            JournalTextMarker::Category(category) => {
                let desired = if *category == state.category {
                    JournalStyle::TEXT_COLOR_SELECTED
                } else {
                    JournalStyle::TEXT_COLOR
                };
                set_text_color(&mut color, desired);
            }
            JournalTextMarker::Day => set_text(&mut text, &day),
            JournalTextMarker::Time => set_text(&mut text, &time),
            JournalTextMarker::Row(index) => {
                let selected = state.focus == JournalFocus::Topics && *index == state.selected;
                let desired = if selected {
                    JournalStyle::TEXT_COLOR_SELECTED
                } else {
                    JournalStyle::TEXT_COLOR
                };
                set_text_color(&mut color, desired);
            }
            JournalTextMarker::Detail => {
                if let Some(detail_text) = &detail_text {
                    set_text(&mut text, detail_text);
                }
            }
            JournalTextMarker::DetailFooter => {
                if let Some(footer) = &detail_footer {
                    set_text(&mut text, footer);
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn rebuild_journal(
    commands: &mut Commands,
    asset_server: &AssetServer,
    definition: &MenuDefinition,
    viewer_back_pic: Option<&str>,
    state: &mut JournalUiState,
    window_size: Vec2,
    scale: f32,
    root: Entity,
) {
    let area = menu_area(definition, window_size, scale);
    let area_size = Vec2::new(area.width, area.height);

    let mut area_commands = commands.spawn((
        ChildOf(root),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(area.left),
            top: Val::Px(area.top),
            width: Val::Px(area.width),
            height: Val::Px(area.height),
            ..default()
        },
    ));
    if let Some(back_pic) = &definition.back_pic {
        let mut image = ImageNode::new(asset_server.load(get_full_texture_path(back_pic)));
        image.color = white_with_alpha(definition.alpha);
        image.image_mode = NodeImageMode::Stretch;
        area_commands.insert(image);
    }
    let area = area_commands.id();

    let main = commands
        .spawn((
            ChildOf(area),
            JournalContainerMarker::Main,
            full_area_node(),
        ))
        .id();

    for category in JournalCategory::ALL {
        let Some(item) = find_item(definition, category.selector_name()) else {
            warn!("Journal menu has no {} selector", category.selector_name());
            continue;
        };
        spawn_menu_text(
            commands,
            main,
            item,
            area_size,
            JournalStyle::CATEGORY_FONT_SIZE * scale,
            JournalStyle::TEXT_COLOR,
            JournalTextMarker::Category(category),
            &menu_text(item.title()),
        );
    }

    for name in ["MENU_ITEM_DAY_TITLE", "MENU_ITEM_TIME_TITLE"] {
        if let Some(item) = find_item(definition, name) {
            spawn_menu_text(
                commands,
                main,
                item,
                area_size,
                JournalStyle::DATETIME_FONT_SIZE * scale,
                JournalStyle::TEXT_COLOR,
                JournalTextMarker::Static,
                &menu_text(item.title()),
            );
        }
    }
    if let Some(item) = find_item(definition, "MENU_ITEM_DAY") {
        spawn_menu_text(
            commands,
            main,
            item,
            area_size,
            JournalStyle::DATETIME_FONT_SIZE * scale,
            JournalStyle::TEXT_COLOR,
            JournalTextMarker::Day,
            "",
        );
    }
    if let Some(item) = find_item(definition, "MENU_ITEM_TIME") {
        spawn_menu_text(
            commands,
            main,
            item,
            area_size,
            JournalStyle::DATETIME_FONT_SIZE * scale,
            JournalStyle::TEXT_COLOR,
            JournalTextMarker::Time,
            "",
        );
    }

    spawn_topic_list(commands, main, definition, state, area_size, scale);

    let detail_text = state.detail_text();
    state.detail_dirty = false;
    spawn_detail(
        commands,
        asset_server,
        viewer_back_pic,
        area,
        area_size,
        scale,
        &detail_text,
    );
}

fn spawn_topic_list(
    commands: &mut Commands,
    parent: Entity,
    definition: &MenuDefinition,
    state: &JournalUiState,
    area_size: Vec2,
    scale: f32,
) {
    let Some(list_item) = find_item(definition, state.category.list_name()) else {
        warn!("Journal menu has no {} listbox", state.category.list_name());
        return;
    };
    let (x, y, width, height) = item_rect(list_item, area_size);
    let pad_x = list_item.frame_size.0 as f32 * area_size.x / MENU_SCRIPT_DIV;
    let pad_y = list_item.frame_size.1 as f32 * area_size.y / MENU_SCRIPT_DIV;

    let list = commands
        .spawn((
            ChildOf(parent),
            JournalListMarker,
            ScrollPosition::default(),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(x + pad_x),
                top: Val::Px(y + pad_y),
                width: Val::Px((width - 2.0 * pad_x).max(0.0)),
                height: Val::Px((height - 2.0 * pad_y).max(0.0)),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                overflow: Overflow::scroll_y(),
                ..default()
            },
        ))
        .id();

    for (index, topic_index) in state.active.iter().enumerate() {
        let Some(topic) = state.topics.get(*topic_index) else {
            continue;
        };
        commands.spawn((
            ChildOf(list),
            JournalTextMarker::Row(index),
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(JournalStyle::ROW_HEIGHT * scale),
                overflow: Overflow::clip(),
                ..default()
            },
            UiText::new(
                topic.name.clone(),
                JournalStyle::TOPIC_FONT_SIZE * scale,
                JournalStyle::TEXT_COLOR,
            ),
        ));
    }
}

fn spawn_detail(
    commands: &mut Commands,
    asset_server: &AssetServer,
    viewer_back_pic: Option<&str>,
    area: Entity,
    area_size: Vec2,
    scale: f32,
    detail_text: &str,
) {
    let mut detail_commands = commands.spawn((
        ChildOf(area),
        JournalContainerMarker::Detail,
        Visibility::Hidden,
        full_area_node(),
    ));
    if let Some(back_pic) = viewer_back_pic {
        let mut image = ImageNode::new(asset_server.load(get_full_texture_path(back_pic)));
        image.image_mode = NodeImageMode::Stretch;
        detail_commands.insert(image);
    }
    let detail = detail_commands.id();

    let content = commands
        .spawn((
            ChildOf(detail),
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                padding: UiRect::axes(
                    Val::Px(JournalStyle::DETAIL_PADDING_X * area_size.x),
                    Val::Px(JournalStyle::DETAIL_PADDING_Y * area_size.y),
                ),
                ..default()
            },
        ))
        .id();

    let scroll = commands
        .spawn((
            ChildOf(content),
            JournalDetailScrollMarker,
            ScrollPosition::default(),
            Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                overflow: Overflow::scroll_y(),
                ..default()
            },
        ))
        .id();
    commands.spawn((
        ChildOf(scroll),
        JournalTextMarker::Detail,
        Node {
            width: Val::Percent(100.0),
            ..default()
        },
        UiText::new(
            detail_text.to_owned(),
            JournalStyle::DETAIL_FONT_SIZE * scale,
            JournalStyle::TEXT_COLOR,
        ),
    ));
    commands.spawn((
        ChildOf(content),
        JournalTextMarker::DetailFooter,
        Node {
            width: Val::Percent(100.0),
            margin: UiRect::top(Val::Px(8.0 * scale)),
            ..default()
        },
        UiText::new(
            String::new(),
            JournalStyle::FOOTER_FONT_SIZE * scale,
            JournalStyle::TEXT_COLOR,
        )
        .justify(Justify::Center),
    ));
}

fn spawn_menu_text(
    commands: &mut Commands,
    parent: Entity,
    item: &MenuItemDefinition,
    area_size: Vec2,
    font_size: f32,
    color: Color,
    marker: JournalTextMarker,
    text: &str,
) {
    let (x, y, width, height) = item_rect(item, area_size);
    let centered = item.flags & MENU_ITEM_FLAG_TXT_CENTER != 0;
    let ui_text = UiText::new(text.to_owned(), font_size, color);
    let ui_text = if centered {
        ui_text.justify(Justify::Center)
    } else {
        ui_text
    };
    commands.spawn((
        ChildOf(parent),
        marker,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(x),
            top: Val::Px(y),
            width: Val::Px(width),
            height: Val::Px(height),
            display: Display::Flex,
            justify_content: if centered {
                JustifyContent::Center
            } else {
                JustifyContent::FlexStart
            },
            align_items: if centered {
                AlignItems::Center
            } else {
                AlignItems::FlexStart
            },
            ..default()
        },
        ui_text,
    ));
}

/// Applies the requested line scroll to the entry viewer and returns the
/// page/percentage footer, clamped to the content height.
#[allow(clippy::type_complexity)]
fn sync_detail_scroll(
    state: &mut JournalUiState,
    scroll: Single<
        (&mut ScrollPosition, &ComputedNode),
        (With<JournalDetailScrollMarker>, Without<JournalListMarker>),
    >,
    scale: f32,
) -> String {
    let (mut scroll, computed) = scroll.into_inner();
    let line_height = JournalStyle::DETAIL_LINE_HEIGHT * scale;
    let viewport_height = computed.size().y;
    let content_height = computed.content_size().y;
    let max_scroll = (content_height - viewport_height).max(0.0);
    let max_lines = (max_scroll / line_height).ceil() as usize;
    if state.detail_scroll > max_lines {
        state.detail_scroll = max_lines;
    }
    let y = (state.detail_scroll as f32 * line_height).min(max_scroll);
    if (scroll.0.y - y).abs() > f32::EPSILON {
        scroll.0.y = y;
    }

    let visible_lines = (viewport_height / line_height).max(1.0);
    let total_lines = (content_height / line_height).ceil().max(1.0);
    let page_count = (total_lines / visible_lines).ceil().max(1.0) as u32;
    let page = ((y / viewport_height.max(1.0)).floor() as u32 + 1).min(page_count);
    let percent = if max_scroll > 0.0 {
        (y / max_scroll * 100.0).round() as u32
    } else {
        100
    };
    format!("{page}/{page_count} ({percent}%)")
}

fn find_item<'a>(definition: &'a MenuDefinition, name: &str) -> Option<&'a MenuItemDefinition> {
    definition.items.iter().find(|item| item.name == name)
}

fn full_area_node() -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(0.0),
        top: Val::Px(0.0),
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        ..default()
    }
}

fn white_with_alpha(alpha: u8) -> Color {
    Color::WHITE.with_alpha(f32::from(alpha) / 255.0)
}

/// Containers must inherit the root's visibility, `Visibility::Visible` would
/// force them visible even after the journal root is hidden.
fn set_inherited_visibility(visibility: &mut Visibility, visible: bool) {
    let desired = if visible {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    if *visibility != desired {
        *visibility = desired;
    }
}

fn time_text(game_time: &GameTime) -> String {
    let hour = game_time.hour();
    let minute = game_time.minutes_of_day() % 60;
    format!("{hour:02}:{minute:02}")
}
