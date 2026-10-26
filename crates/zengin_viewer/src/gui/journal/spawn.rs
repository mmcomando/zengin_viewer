use bevy::prelude::*;

use crate::ui_kit::{UiText, spawn_gui_root};

use super::state::JournalCategory;
use super::style::JournalStyle;

#[derive(Component)]
pub(super) enum JournalContainerMarker {
    Root,
    Main,
    Detail,
}

#[derive(Component)]
pub(super) struct JournalCategoryBoxMarker(pub(super) JournalCategory);

#[derive(Component)]
pub(super) struct JournalListMarker;

#[derive(Component)]
pub(super) struct JournalDetailScrollMarker;

/// Text node kind, so one query can update all journal texts without aliasing
#[derive(Component)]
pub(super) enum JournalTextMarker {
    Category(JournalCategory),
    DayTime,
    Row(usize),
    Detail,
    DetailFooter,
}

pub(super) fn spawn_journal_ui(mut commands: Commands) {
    let root = spawn_gui_root(&mut commands, JournalContainerMarker::Root);
    commands.entity(root).insert(GlobalZIndex(10));
    spawn_main_panel(&mut commands, root);
    spawn_detail_panel(&mut commands, root);
}

fn spawn_main_panel(commands: &mut Commands, root: Entity) {
    let panel = commands
        .spawn((
            ChildOf(root),
            JournalContainerMarker::Main,
            Visibility::Inherited,
            BackgroundColor(JournalStyle::PANEL_BACKGROUND),
            BorderColor::all(JournalStyle::BORDER_COLOR),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(4.0),
                top: Val::Percent(8.0),
                width: Val::Percent(92.0),
                height: Val::Percent(84.0),
                display: Display::Flex,
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(8.0),
                padding: UiRect::all(Val::Px(10.0)),
                border: UiRect::all(Val::Px(JournalStyle::BORDER_WIDTH)),
                ..default()
            },
        ))
        .id();

    let left = commands
        .spawn((
            ChildOf(panel),
            Node {
                width: Val::Percent(26.0),
                height: Val::Percent(100.0),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                ..default()
            },
        ))
        .id();
    for category in JournalCategory::ALL {
        spawn_category_box(commands, left, category);
    }
    spawn_day_time_box(commands, left);

    commands.spawn((
        ChildOf(panel),
        JournalListMarker,
        ScrollPosition::default(),
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.35)),
        BorderColor::all(JournalStyle::BORDER_COLOR),
        Node {
            flex_grow: 1.0,
            height: Val::Percent(100.0),
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(16.0)),
            border: UiRect::all(Val::Px(JournalStyle::BORDER_WIDTH)),
            overflow: Overflow::scroll_y(),
            ..default()
        },
    ));
}

fn spawn_category_box(commands: &mut Commands, parent: Entity, category: JournalCategory) {
    let category_box = commands
        .spawn((
            ChildOf(parent),
            JournalCategoryBoxMarker(category),
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.65)),
            BorderColor::all(JournalStyle::BORDER_COLOR),
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(JournalStyle::CATEGORY_HEIGHT),
                display: Display::Flex,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(JournalStyle::BORDER_WIDTH)),
                ..default()
            },
        ))
        .id();
    commands.spawn((
        ChildOf(category_box),
        JournalTextMarker::Category(category),
        UiText::new(
            category.label(),
            JournalStyle::LABEL_FONT_SIZE,
            JournalStyle::LABEL_COLOR,
        )
        .justify(Justify::Center),
    ));
}

fn spawn_day_time_box(commands: &mut Commands, parent: Entity) {
    let box_entity = commands
        .spawn((
            ChildOf(parent),
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.65)),
            BorderColor::all(JournalStyle::BORDER_COLOR),
            Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::FlexEnd,
                align_items: AlignItems::FlexEnd,
                padding: UiRect::all(Val::Px(12.0)),
                border: UiRect::all(Val::Px(JournalStyle::BORDER_WIDTH)),
                ..default()
            },
        ))
        .id();
    commands.spawn((
        ChildOf(box_entity),
        JournalTextMarker::DayTime,
        UiText::new(
            String::new(),
            JournalStyle::LABEL_FONT_SIZE,
            JournalStyle::LABEL_COLOR_SELECTED,
        )
        .justify(Justify::Right),
    ));
}

fn spawn_detail_panel(commands: &mut Commands, root: Entity) {
    let panel = commands
        .spawn((
            ChildOf(root),
            JournalContainerMarker::Detail,
            Visibility::Hidden,
            BackgroundColor(JournalStyle::PANEL_BACKGROUND),
            BorderColor::all(JournalStyle::BORDER_COLOR),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(4.0),
                top: Val::Percent(8.0),
                width: Val::Percent(92.0),
                height: Val::Percent(84.0),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(40.0)),
                border: UiRect::all(Val::Px(JournalStyle::BORDER_WIDTH)),
                ..default()
            },
        ))
        .id();

    let scroll = commands
        .spawn((
            ChildOf(panel),
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
            String::new(),
            JournalStyle::DETAIL_FONT_SIZE,
            JournalStyle::DETAIL_COLOR,
        ),
    ));
    commands.spawn((
        ChildOf(panel),
        JournalTextMarker::DetailFooter,
        Node {
            width: Val::Percent(100.0),
            margin: UiRect::top(Val::Px(12.0)),
            ..default()
        },
        UiText::new(
            String::new(),
            JournalStyle::FOOTER_FONT_SIZE,
            JournalStyle::FOOTER_COLOR,
        )
        .justify(Justify::Center),
    ));
}
