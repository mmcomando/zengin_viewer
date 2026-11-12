use bevy::color::Alpha;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::gui::menu_screen::{MenuScreenTextMarker, spawn_menu_screen};
use crate::ui_kit::{UiText, set_text, set_text_color, set_visibility};
use crate::zengin::{
    CurrentWorld, MENU_FLAG_SHOW_INFO, MENU_ITEM_FLAG_DISABLED, MenuDefinition, MenuItemDefinition,
    get_full_texture_path,
};

use super::spawn::{MainMenuContentMarker, MainMenuRootMarker};
use super::state::{MainMenuData, MainMenuState, first_selectable, is_selectable};
use super::style::MainMenuStyle;

#[derive(Component)]
pub(super) struct MainMenuInfoTextMarker;

/// Engine start screen drawn behind the main menu, outside MENU.DAT.
const MAIN_MENU_BACKGROUND: &str = "startscreen.tga";

#[allow(clippy::type_complexity)]
pub(super) fn sync_main_menu(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    data: Option<Res<MainMenuData>>,
    current_world: Res<CurrentWorld>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut state: ResMut<MainMenuState>,
    mut root: Single<&mut Visibility, With<MainMenuRootMarker>>,
    content: Single<Entity, With<MainMenuContentMarker>>,
    mut texts: Query<(&MenuScreenTextMarker, &mut TextColor)>,
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
    let in_game = !current_world.0.is_empty();
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
        if !is_selectable(data, state.selected_slot, in_game) {
            state.selected_slot = first_selectable(data, in_game).unwrap_or(state.selected_slot);
        }
        rebuild_menu(
            &mut commands,
            &asset_server,
            data,
            window_size,
            ui_scale,
            &state,
            in_game,
            *content,
        );
        state.last_size = window_size;
        state.last_scale = ui_scale;
        state.rebuild = false;
    }

    for (marker, mut color) in &mut texts {
        let item = data.items.iter().find(|item| item.slot == marker.slot);
        let value = item_color(item, marker.slot == state.selected_slot, in_game);
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
    in_game: bool,
    content: Entity,
) {
    commands.entity(content).despawn_children();

    spawn_background(commands, asset_server, content);

    let area_id = spawn_menu_screen(
        commands,
        asset_server,
        data,
        content,
        window_size,
        scale,
        MainMenuStyle::ITEM_FONT_SIZE * scale,
        MainMenuStyle::MENU_AREA_FALLBACK,
        |item| item.title().to_owned(),
        |item| item_color(Some(item), item.slot == state.selected_slot, in_game),
    );

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

fn item_color(item: Option<&MenuItemDefinition>, selected: bool, in_game: bool) -> Color {
    let alpha = item.map_or(255, |item| item.alpha);
    let base = if item.is_some_and(|item| {
        item.flags & MENU_ITEM_FLAG_DISABLED != 0
            || (item.is_only_in_game() && !in_game)
            || (item.is_only_out_game() && in_game)
    }) {
        MainMenuStyle::TEXT_COLOR_DISABLED
    } else if selected {
        MainMenuStyle::TEXT_COLOR_SELECTED
    } else {
        MainMenuStyle::TEXT_COLOR
    };
    base.with_alpha(f32::from(alpha) / 255.0)
}
