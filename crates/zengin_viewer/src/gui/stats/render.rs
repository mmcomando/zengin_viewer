use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::gui::menu_screen::spawn_menu_screen;
use crate::ui_kit::set_visibility;

use super::data::StatsMenuData;
use super::spawn::{StatsContentMarker, StatsRootMarker};
use super::state::StatsUiState;
use super::style::StatsStyle;

pub(super) fn sync_stats_ui(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    data: Res<StatsMenuData>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut state: ResMut<StatsUiState>,
    mut root: Single<&mut Visibility, With<StatsRootMarker>>,
    content: Single<Entity, With<StatsContentMarker>>,
) {
    let open = state.is_open();
    set_visibility(&mut root, open);
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
    if !state.take_rebuild(window_size, scale) {
        return;
    }
    let Some(snapshot) = state.snapshot() else {
        return;
    };

    commands.entity(*content).despawn_children();
    spawn_menu_screen(
        &mut commands,
        &asset_server,
        definition,
        *content,
        window_size,
        scale,
        StatsStyle::FONT_SIZE * scale,
        StatsStyle::MENU_AREA_FALLBACK,
        |item| snapshot.text_for(item),
        StatsStyle::text_color,
    );
}
