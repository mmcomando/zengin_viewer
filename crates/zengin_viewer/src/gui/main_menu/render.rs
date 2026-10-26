use bevy::color::Alpha;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::ui_kit::{UiText, set_text, set_text_color, set_visibility};
use crate::zengin::{
    MENU_FLAG_ALIGN_CENTER, MENU_FLAG_DONT_SCALE_DIM, MENU_FLAG_SHOW_INFO, MENU_ITEM_FLAG_DISABLED,
    MENU_ITEM_FLAG_TXT_CENTER, MENU_SCRIPT_DIV, MenuDefinition, MenuItemDefinition,
    get_full_texture_path,
};

use super::spawn::{MainMenuContentMarker, MainMenuRootMarker};
use super::state::{MainMenuData, MainMenuState, first_selectable, is_selectable};
use super::style::MainMenuStyle;

#[derive(Component)]
pub(super) struct MainMenuTextMarker {
    pub(super) slot: usize,
}

#[derive(Component)]
pub(super) struct MainMenuInfoTextMarker;

struct MenuArea {
    left: f32,
    top: f32,
    width: f32,
    height: f32,
}

/// Engine start screen drawn behind the main menu, outside MENU.DAT.
const MAIN_MENU_BACKGROUND: &str = "startscreen.tga";

#[allow(clippy::type_complexity)]
pub(super) fn sync_main_menu(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    data: Option<Res<MainMenuData>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut state: ResMut<MainMenuState>,
    mut root: Single<&mut Visibility, With<MainMenuRootMarker>>,
    content: Single<Entity, With<MainMenuContentMarker>>,
    mut texts: Query<(&MainMenuTextMarker, &mut TextColor)>,
    mut info_texts: Query<
        (&mut Text, &mut Visibility),
        (With<MainMenuInfoTextMarker>, Without<MainMenuRootMarker>),
    >,
) {
    set_visibility(&mut root, state.open);
    if !state.open {
        return;
    }
    let Some(data) = data.as_ref().and_then(|data| data.0.as_ref()) else {
        return;
    };
    let (window_size, ui_scale) =
        windows
            .iter()
            .next()
            .map_or((Vec2::new(640.0, 480.0), 1.0), |window| {
                (
                    Vec2::new(window.width(), window.height()),
                    window.scale_factor(),
                )
            });

    if state.rebuild
        || state.last_size != window_size
        || (state.last_scale - ui_scale).abs() > f32::EPSILON
    {
        if !is_selectable(data, state.selected_slot) {
            state.selected_slot = first_selectable(data).unwrap_or(state.selected_slot);
        }
        rebuild_menu(
            &mut commands,
            &asset_server,
            data,
            window_size,
            ui_scale,
            &state,
            *content,
        );
        state.last_size = window_size;
        state.last_scale = ui_scale;
        state.rebuild = false;
    }

    for (marker, mut color) in &mut texts {
        let item = data.items.iter().find(|item| item.slot == marker.slot);
        let value = item_color(item, marker.slot == state.selected_slot);
        set_text_color(&mut color, value);
    }

    let info = state
        .selected_item(data)
        .and_then(|item| item.text.get(1))
        .map(String::as_str)
        .unwrap_or_default();
    if let Some((mut text, mut visibility)) = info_texts.iter_mut().next() {
        set_text(&mut text, info);
        let desired = if info.is_empty() {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        if *visibility != desired {
            *visibility = desired;
        }
    }
}

fn rebuild_menu(
    commands: &mut Commands,
    asset_server: &AssetServer,
    data: &MenuDefinition,
    window_size: Vec2,
    scale: f32,
    state: &MainMenuState,
    content: Entity,
) {
    commands.entity(content).despawn_children();

    let area = menu_area(data, window_size, scale);

    spawn_background(commands, asset_server, content);

    let mut area_commands = commands.spawn((
        ChildOf(content),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(area.left),
            top: Val::Px(area.top),
            width: Val::Px(area.width),
            height: Val::Px(area.height),
            ..default()
        },
    ));
    if let Some(back_pic) = &data.back_pic {
        let mut image = ImageNode::new(asset_server.load(get_full_texture_path(back_pic)));
        image.color = white_with_alpha(data.alpha);
        image.image_mode = NodeImageMode::Stretch;
        area_commands.insert(image);
    } else {
        area_commands.insert(BackgroundColor(MainMenuStyle::MENU_AREA_FALLBACK));
    }
    let area_id = area_commands.id();

    for item in &data.items {
        spawn_item(
            commands,
            asset_server,
            item,
            area_id,
            Vec2::new(area.width, area.height),
            scale,
            state.selected_slot,
        );
    }

    if data.flags & MENU_FLAG_SHOW_INFO != 0 {
        spawn_info_text(commands, area_id, scale);
    }
}

fn spawn_background(commands: &mut Commands, asset_server: &AssetServer, content: Entity) {
    let mut image = ImageNode::new(asset_server.load(get_full_texture_path(MAIN_MENU_BACKGROUND)));
    image.color = Color::WHITE;
    image.image_mode = NodeImageMode::Stretch;
    commands.spawn((
        ChildOf(content),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        image,
    ));
}

fn menu_area(data: &MenuDefinition, window_size: Vec2, scale: f32) -> MenuArea {
    let (width, height) = if data.flags & MENU_FLAG_DONT_SCALE_DIM != 0 {
        (
            data.dim.0 as f32 * 640.0 * scale / MENU_SCRIPT_DIV,
            data.dim.1 as f32 * 480.0 * scale / MENU_SCRIPT_DIV,
        )
    } else {
        (
            data.dim.0 as f32 * window_size.x / MENU_SCRIPT_DIV,
            data.dim.1 as f32 * window_size.y / MENU_SCRIPT_DIV,
        )
    };
    let (left, top) = if data.flags & MENU_FLAG_ALIGN_CENTER != 0 {
        (
            (window_size.x - width) / 2.0,
            (window_size.y - height) / 2.0,
        )
    } else {
        (
            data.pos.0 as f32 * 640.0 * scale / MENU_SCRIPT_DIV,
            data.pos.1 as f32 * 480.0 * scale / MENU_SCRIPT_DIV,
        )
    };
    MenuArea {
        left,
        top,
        width,
        height,
    }
}

fn spawn_item(
    commands: &mut Commands,
    asset_server: &AssetServer,
    item: &MenuItemDefinition,
    area: Entity,
    area_size: Vec2,
    scale: f32,
    selected_slot: usize,
) {
    let x = item.pos.0 as f32 * area_size.x / MENU_SCRIPT_DIV;
    let y = item.pos.1 as f32 * area_size.y / MENU_SCRIPT_DIV;
    let width = item.dim.0 as f32 * area_size.x / MENU_SCRIPT_DIV;
    let height = item.dim.1 as f32 * area_size.y / MENU_SCRIPT_DIV;

    let mut row = commands.spawn((
        ChildOf(area),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(x),
            top: Val::Px(y),
            width: Val::Px(width.max(0.0)),
            height: Val::Px(height.max(0.0)),
            display: Display::Flex,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
    ));
    if let Some(back_pic) = &item.back_pic {
        let mut image = ImageNode::new(asset_server.load(get_full_texture_path(back_pic)));
        image.color = white_with_alpha(item.alpha);
        image.image_mode = NodeImageMode::Stretch;
        row.insert(image);
    }
    if item.title().is_empty() {
        return;
    }

    let color = item_color(Some(item), item.slot == selected_slot);
    let text = UiText::new(item.title(), MainMenuStyle::ITEM_FONT_SIZE * scale, color);
    let text = if item.flags & MENU_ITEM_FLAG_TXT_CENTER != 0 {
        text.justify(Justify::Center)
    } else {
        text
    };
    let row_id = row.id();
    commands.spawn((
        ChildOf(row_id),
        MainMenuTextMarker { slot: item.slot },
        text,
    ));
}

fn spawn_info_text(commands: &mut Commands, area: Entity, scale: f32) {
    commands.spawn((
        ChildOf(area),
        MainMenuInfoTextMarker,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            right: Val::Px(0.0),
            bottom: Val::Px(MainMenuStyle::INFO_BOTTOM_OFFSET * scale),
            ..default()
        },
        UiText::new(
            String::new(),
            MainMenuStyle::INFO_FONT_SIZE * scale,
            MainMenuStyle::TEXT_COLOR,
        )
        .justify(Justify::Center),
    ));
}

fn item_color(item: Option<&MenuItemDefinition>, selected: bool) -> Color {
    let alpha = item.map_or(255, |item| item.alpha);
    let base = if item
        .is_some_and(|item| item.flags & MENU_ITEM_FLAG_DISABLED != 0 || item.is_only_in_game())
    {
        MainMenuStyle::TEXT_COLOR_DISABLED
    } else if selected {
        MainMenuStyle::TEXT_COLOR_SELECTED
    } else {
        MainMenuStyle::TEXT_COLOR
    };
    base.with_alpha(f32::from(alpha) / 255.0)
}

fn white_with_alpha(alpha: u8) -> Color {
    Color::WHITE.with_alpha(f32::from(alpha) / 255.0)
}
