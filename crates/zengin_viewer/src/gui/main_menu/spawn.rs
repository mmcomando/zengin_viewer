use bevy::prelude::*;

use crate::ui_kit::spawn_gui_root;

use super::style::MainMenuStyle;

#[derive(Component)]
pub(super) struct MainMenuRootMarker;

#[derive(Component)]
pub(super) struct MainMenuContentMarker;

pub(super) fn spawn_main_menu(mut commands: Commands) {
    let root = spawn_gui_root(&mut commands, MainMenuRootMarker);
    commands.entity(root).insert((
        GlobalZIndex(100),
        BackgroundColor(MainMenuStyle::BACKGROUND),
    ));

    commands.spawn((
        ChildOf(root),
        MainMenuContentMarker,
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
    ));
}
