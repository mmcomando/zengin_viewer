use std::path::{Path, PathBuf};

use bevy::asset::RenderAssetUsages;
use bevy::color::Alpha;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::gui::menu_screen::{MenuScreenTextMarker, item_rect, menu_area, spawn_menu_screen};
use crate::zengin::save::SaveEntry;
use crate::zengin::{
    CurrentWorld, MENU_FLAG_SHOW_INFO, MenuDefinition, MenuItemDefinition, get_full_texture_path,
};
use dev_util::ui_kit::{UiText, set_text, set_text_color, set_visibility};

use super::spawn::{MainMenuContentMarker, MainMenuRootMarker};
use super::state::{
    LoadMenuData, MainMenuData, MainMenuScreen, MainMenuState, is_disabled, is_load_back,
    load_selectable_slots, load_slot_index, selectable_slots,
};
use super::style::MainMenuStyle;
use dev_util::perf_debug;

#[derive(Component)]
pub(super) struct MainMenuInfoTextMarker;

/// Overlay image showing the highlighted save's screenshot.
#[derive(Component)]
pub(super) struct SaveThumbnailMarker;

/// Last loaded save thumbnail, so the image is decoded only when the selection
/// changes.
#[derive(Resource, Default)]
pub(super) struct SaveThumbnailCache {
    path: Option<PathBuf>,
    handle: Option<Handle<Image>>,
}

/// Engine start screen drawn behind the main menu, outside MENU.DAT.
const MAIN_MENU_BACKGROUND: &str = "startscreen.tga";
/// Label of a load-screen row without a save file.
const EMPTY_SLOT_TEXT: &str = "---";

const THUMBNAIL_ITEM: &str = "MENUITEM_LOADSAVE_THUMBPIC";
const LEVELNAME_VALUE_ITEM: &str = "MENUITEM_LOADSAVE_LEVELNAME_VALUE";
const DATETIME_VALUE_ITEM: &str = "MENUITEM_LOADSAVE_DATETIME_VALUE";
const GAMETIME_VALUE_ITEM: &str = "MENUITEM_LOADSAVE_GAMETIME_VALUE";
const PLAYTIME_VALUE_ITEM: &str = "MENUITEM_LOADSAVE_PLAYTIME_VALUE";

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn sync_main_menu(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    data: Option<Res<MainMenuData>>,
    load_data: Option<Res<LoadMenuData>>,
    current_world: Res<CurrentWorld>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut state: ResMut<MainMenuState>,
    mut root: Single<&mut Visibility, With<MainMenuRootMarker>>,
    content: Single<Entity, With<MainMenuContentMarker>>,
    mut texts: Query<
        (&MenuScreenTextMarker, &mut Text, &mut TextColor),
        Without<MainMenuInfoTextMarker>,
    >,
    mut images: ResMut<Assets<Image>>,
    mut thumbnail: ResMut<SaveThumbnailCache>,
    mut thumbnail_nodes: Query<&mut ImageNode, With<SaveThumbnailMarker>>,
    mut info_texts: Query<
        (&mut Text, &mut Visibility),
        (With<MainMenuInfoTextMarker>, Without<MainMenuRootMarker>),
    >,
) {
    let _perf = perf_debug!();
    set_visibility(&mut root, state.open);
    if !state.open {
        return;
    }
    let Some(definition) = active_definition(&state, data.as_ref(), load_data.as_ref()) else {
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

    update_thumbnail(&mut images, &mut thumbnail, &state, definition);

    if state.rebuild
        || state.last_size != window_size
        || (state.last_scale - ui_scale).abs() > f32::EPSILON
    {
        let selectable = active_selectable_slots(&state, definition, in_game);
        if !selectable.contains(&state.selected_slot)
            && let Some(first) = selectable.first()
        {
            state.selected_slot = *first;
        }
        update_thumbnail(&mut images, &mut thumbnail, &state, definition);
        rebuild_menu(
            &mut commands,
            &asset_server,
            definition,
            window_size,
            ui_scale,
            &state,
            in_game,
            thumbnail.handle.clone(),
            *content,
        );
        state.last_size = window_size;
        state.last_scale = ui_scale;
        state.rebuild = false;
    }

    for mut image in &mut thumbnail_nodes {
        match &thumbnail.handle {
            Some(handle) => {
                if image.image != *handle {
                    image.image = handle.clone();
                }
                image.color = Color::WHITE;
            }
            None => image.color = Color::NONE,
        }
    }

    for (marker, mut text, mut color) in &mut texts {
        let item = definition
            .items
            .iter()
            .find(|item| item.slot == marker.slot);
        if let Some(item) = item {
            let value = screen_item_text(item, &state, definition);
            set_text(&mut text, &value);
        }
        let value = screen_item_color(
            item,
            marker.slot == state.selected_slot,
            &state,
            definition,
            in_game,
        );
        set_text_color(&mut color, value);
    }

    let info = state
        .selected_item(definition)
        .and_then(|item| item.text.get(1))
        .map_or_default(String::as_str);
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

fn active_definition<'a>(
    state: &MainMenuState,
    main: Option<&'a Res<MainMenuData>>,
    load: Option<&'a Res<LoadMenuData>>,
) -> Option<&'a MenuDefinition> {
    match state.screen {
        MainMenuScreen::Main => main?.0.as_ref(),
        MainMenuScreen::Load => load?.0.as_ref(),
    }
}

fn active_selectable_slots(
    state: &MainMenuState,
    definition: &MenuDefinition,
    in_game: bool,
) -> Vec<usize> {
    match state.screen {
        MainMenuScreen::Main => selectable_slots(definition, in_game),
        MainMenuScreen::Load => load_selectable_slots(definition, &state.saved_games),
    }
}

#[allow(clippy::too_many_arguments)]
fn rebuild_menu(
    commands: &mut Commands,
    asset_server: &AssetServer,
    definition: &MenuDefinition,
    window_size: Vec2,
    scale: f32,
    state: &MainMenuState,
    in_game: bool,
    thumbnail: Option<Handle<Image>>,
    content: Entity,
) {
    commands.entity(content).despawn_children();

    if state.screen == MainMenuScreen::Main {
        spawn_background(commands, asset_server, content);
    }

    let area_id = spawn_menu_screen(
        commands,
        asset_server,
        definition,
        content,
        window_size,
        scale,
        match state.screen {
            MainMenuScreen::Main => MainMenuStyle::ITEM_FONT_SIZE * scale,
            MainMenuScreen::Load => MainMenuStyle::LOAD_FONT_SIZE * scale,
        },
        MainMenuStyle::MENU_AREA_FALLBACK,
        |item| screen_item_text(item, state, definition),
        |item| {
            screen_item_color(
                Some(item),
                item.slot == state.selected_slot,
                state,
                definition,
                in_game,
            )
        },
    );

    if state.screen == MainMenuScreen::Load {
        spawn_thumbnail_overlay(commands, definition, area_id, window_size, scale, thumbnail);
    }
    if definition.flags & MENU_FLAG_SHOW_INFO != 0 {
        let info_font_size = match state.screen {
            MainMenuScreen::Main => MainMenuStyle::INFO_FONT_SIZE,
            MainMenuScreen::Load => MainMenuStyle::LOAD_INFO_FONT_SIZE,
        };
        spawn_info_text(commands, area_id, scale, info_font_size);
    }
}

fn screen_item_text(
    item: &MenuItemDefinition,
    state: &MainMenuState,
    definition: &MenuDefinition,
) -> String {
    if state.screen == MainMenuScreen::Main {
        return item.title().to_owned();
    }
    let selected = selected_load_entry(state, definition);
    if let Some(text) = metadata_value_text(item, selected) {
        return text;
    }
    if is_metadata_value_item(item) {
        return String::new();
    }
    let Some(index) = load_slot_index(definition, item.slot) else {
        return item.title().to_owned();
    };
    state
        .saved_games
        .get(index)
        .map_or_else(|| EMPTY_SLOT_TEXT.to_owned(), row_label)
}

fn row_label(entry: &SaveEntry) -> String {
    format!(
        "{}  {}",
        crate::zengin::save::display_name(&entry.path),
        short_world_name(&entry.metadata.world_name)
    )
}

/// The save panel shows only the final world path segment
/// (`NEWWORLD/NEWWORLD` -> `NEWWORLD`).
fn short_world_name(world_name: &str) -> &str {
    world_name.rsplit(['/', '\\']).next().unwrap_or(world_name)
}

fn metadata_value_text(item: &MenuItemDefinition, entry: Option<&SaveEntry>) -> Option<String> {
    let entry = entry?;
    Some(match item.name.as_str() {
        LEVELNAME_VALUE_ITEM => short_world_name(&entry.metadata.world_name).to_owned(),
        DATETIME_VALUE_ITEM => entry.metadata.saved_at_label(),
        GAMETIME_VALUE_ITEM => entry.metadata.game_time_label(),
        PLAYTIME_VALUE_ITEM => entry.metadata.play_time_label(),
        _ => return None,
    })
}

fn is_metadata_value_item(item: &MenuItemDefinition) -> bool {
    matches!(
        item.name.as_str(),
        LEVELNAME_VALUE_ITEM | DATETIME_VALUE_ITEM | GAMETIME_VALUE_ITEM | PLAYTIME_VALUE_ITEM
    )
}

fn selected_load_entry<'a>(
    state: &'a MainMenuState,
    definition: &MenuDefinition,
) -> Option<&'a SaveEntry> {
    if state.screen != MainMenuScreen::Load {
        return None;
    }
    let index = load_slot_index(definition, state.selected_slot)?;
    state.saved_games.get(index)
}

fn update_thumbnail(
    images: &mut Assets<Image>,
    cache: &mut SaveThumbnailCache,
    state: &MainMenuState,
    definition: &MenuDefinition,
) {
    let entry = selected_load_entry(state, definition);
    let path = entry.and_then(|entry| {
        entry
            .metadata
            .screenshot
            .as_ref()
            .map(|name| entry.path.join(name))
    });
    if cache.path == path {
        return;
    }
    cache.path.clone_from(&path);
    cache.handle = path
        .as_deref()
        .and_then(|path| load_thumbnail_image(images, path));
}

fn load_thumbnail_image(images: &mut Assets<Image>, path: &Path) -> Option<Handle<Image>> {
    let image = image::open(path).ok()?;
    Some(images.add(Image::from_dynamic(
        image,
        true,
        RenderAssetUsages::default(),
    )))
}

fn spawn_thumbnail_overlay(
    commands: &mut Commands,
    definition: &MenuDefinition,
    area: Entity,
    window_size: Vec2,
    scale: f32,
    handle: Option<Handle<Image>>,
) {
    let Some(item) = definition
        .items
        .iter()
        .find(|item| item.name == THUMBNAIL_ITEM)
    else {
        return;
    };
    let area_rect = menu_area(definition, window_size, scale);
    let (x, y, width, height) = item_rect(item, Vec2::new(area_rect.width, area_rect.height));
    let has_image = handle.is_some();
    let mut image = handle.map_or_else(ImageNode::default, ImageNode::new);
    image.image_mode = NodeImageMode::Stretch;
    if !has_image {
        image.color = Color::NONE;
    }
    commands.spawn((
        ChildOf(area),
        SaveThumbnailMarker,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(x),
            top: Val::Px(y),
            width: Val::Px(width),
            height: Val::Px(height),
            ..default()
        },
        image,
    ));
}

fn screen_item_color(
    item: Option<&MenuItemDefinition>,
    selected: bool,
    state: &MainMenuState,
    definition: &MenuDefinition,
    in_game: bool,
) -> Color {
    if state.screen == MainMenuScreen::Main {
        return item_color(item, selected, in_game);
    }
    let alpha = item.map_or(255, |item| item.alpha);
    let enabled = item.is_some_and(|item| load_item_enabled(item, state, definition));
    let base = if !enabled {
        MainMenuStyle::TEXT_COLOR_DISABLED
    } else if selected {
        MainMenuStyle::TEXT_COLOR_SELECTED
    } else {
        MainMenuStyle::TEXT_COLOR
    };
    base.with_alpha(f32::from(alpha) / 255.0)
}

fn load_item_enabled(
    item: &MenuItemDefinition,
    state: &MainMenuState,
    definition: &MenuDefinition,
) -> bool {
    if is_load_back(item) {
        return true;
    }
    load_slot_index(definition, item.slot)
        .is_none_or(|index| state.saved_games.get(index).is_some())
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

fn spawn_info_text(commands: &mut Commands, area: Entity, scale: f32, font_size: f32) {
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
        UiText::new(String::new(), font_size * scale, MainMenuStyle::TEXT_COLOR)
            .justify(Justify::Center),
    ));
}

fn item_color(item: Option<&MenuItemDefinition>, selected: bool, in_game: bool) -> Color {
    let alpha = item.map_or(255, |item| item.alpha);
    let base = if item.is_some_and(|item| is_disabled(item, in_game)) {
        MainMenuStyle::TEXT_COLOR_DISABLED
    } else if selected {
        MainMenuStyle::TEXT_COLOR_SELECTED
    } else {
        MainMenuStyle::TEXT_COLOR
    };
    base.with_alpha(f32::from(alpha) / 255.0)
}
