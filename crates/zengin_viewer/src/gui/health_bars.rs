//! Health bars of the player (persistent, bottom left) and of the selected
//! npc (top center, shown while an npc is selected). Plain Bevy UI nodes
//! updated from the `NpcHealth` components.

use bevy::prelude::*;

use crate::gameplay::objects::PlayerCharacter;
use crate::zengin::{NpcHealth, Selection, SelectionKind};

/// Size and look of the bars.
const BAR_WIDTH: f32 = 200.0;
const BAR_HEIGHT: f32 = 16.0;
const BAR_BACKGROUND: Color = Color::srgba(0.05, 0.02, 0.02, 0.8);
const BAR_BORDER: Color = Color::srgba(0.2, 0.15, 0.1, 0.9);
const HEALTH_FILL: Color = Color::srgb(0.75, 0.08, 0.08);
const TEXT_FONT_SIZE: f32 = 12.0;

/// Marker of the player health bar, on the container and its children.
#[derive(Component, Clone, Copy)]
struct PlayerHealthBar;

/// Marker of the selected-npc health bar, on the container and its children.
#[derive(Component, Clone, Copy)]
struct TargetHealthBar;

/// Red fill of a health bar, its width tracks the remaining hitpoints.
#[derive(Component)]
struct HealthBarFill;

/// `hp / max` text of a health bar. Text entities carry a `Node` too, so the
/// bar container queries must exclude them with this marker.
#[derive(Component)]
struct HealthBarText;

/// Flex container centering the text of a health bar.
#[derive(Component)]
struct HealthBarTextContainer;

/// Node of the bar container (excludes the fill/text child nodes).
type ContainerFilter = (
    Without<HealthBarFill>,
    Without<HealthBarText>,
    Without<HealthBarTextContainer>,
);

pub struct HealthBarsPlugin;

impl Plugin for HealthBarsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_health_bars);
        app.add_systems(Update, (update_player_bar, update_target_bar).chain());
    }
}

fn spawn_health_bars(mut commands: Commands) {
    // Player bar, bottom left corner; the game log sits above it
    commands
        .spawn((
            PlayerHealthBar,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(12.0),
                bottom: Val::Px(12.0),
                width: Val::Px(BAR_WIDTH),
                height: Val::Px(BAR_HEIGHT),
                border: UiRect::all(Val::Px(1.0)),
                // Hidden until the player health is seeded
                display: Display::None,
                ..default()
            },
            BackgroundColor(BAR_BACKGROUND),
            BorderColor::all(BAR_BORDER),
        ))
        .with_children(|parent| spawn_bar_children(parent, PlayerHealthBar));

    // Target bar, centered on the top edge
    commands
        .spawn((
            TargetHealthBar,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(50.0),
                top: Val::Px(12.0),
                margin: UiRect {
                    left: Val::Px(-BAR_WIDTH / 2.0),
                    ..default()
                },
                width: Val::Px(BAR_WIDTH),
                height: Val::Px(BAR_HEIGHT),
                border: UiRect::all(Val::Px(1.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(BAR_BACKGROUND),
            BorderColor::all(BAR_BORDER),
        ))
        .with_children(|parent| spawn_bar_children(parent, TargetHealthBar));
}

/// Spawns the fill and the text children of a health bar container.
fn spawn_bar_children<M: Component + Copy>(parent: &mut ChildSpawnerCommands, marker: M) {
    parent.spawn((
        marker,
        HealthBarFill,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        BackgroundColor(HEALTH_FILL),
    ));
    // The text must be a child of a fixed-size flex node to be vertically
    // centered; horizontal centering comes from its text layout
    parent
        .spawn((
            marker,
            HealthBarTextContainer,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
        ))
        .with_children(|text_container| {
            text_container.spawn((
                marker,
                HealthBarText,
                Text::new(""),
                TextLayout::default().with_justify(Justify::Center),
                TextFont {
                    font_size: TEXT_FONT_SIZE.into(),
                    ..default()
                },
                TextColor(Color::WHITE),
                TextShadow {
                    offset: Vec2::new(1.0, 1.0),
                    color: Color::BLACK,
                },
            ));
        });
}

/// Updates the player bar from the player's npc health.
fn update_player_bar(
    container: Single<&mut Node, (With<PlayerHealthBar>, ContainerFilter)>,
    fill: Single<&mut Node, (With<HealthBarFill>, With<PlayerHealthBar>)>,
    text: Single<&mut Text, (With<HealthBarText>, With<PlayerHealthBar>)>,
    player: Single<&NpcHealth, With<PlayerCharacter>>,
) {
    let health = player.into_inner();
    let mut container = container.into_inner();
    // The player bar is visible once the health is seeded
    if container.display == Display::None {
        container.display = Display::Flex;
    }
    update_bar(fill.into_inner(), text.into_inner(), health);
}

/// Shows the target bar with the selected npc's health, hides it otherwise.
fn update_target_bar(
    container: Single<&mut Node, (With<TargetHealthBar>, ContainerFilter)>,
    fill: Single<&mut Node, (With<HealthBarFill>, With<TargetHealthBar>)>,
    text: Single<&mut Text, (With<HealthBarText>, With<TargetHealthBar>)>,
    selection: Res<Selection>,
    npcs: Query<&NpcHealth>,
) {
    let mut container = container.into_inner();
    let health = (selection.kind == Some(SelectionKind::Npc))
        .then_some(selection.target)
        .flatten()
        .and_then(|target| npcs.get(target).ok());
    let Some(health) = health else {
        if container.display != Display::None {
            container.display = Display::None;
        }
        return;
    };
    if container.display == Display::None {
        container.display = Display::Flex;
    }
    update_bar(fill.into_inner(), text.into_inner(), health);
}

/// Sets the fill width and the text of one bar from `health`.
fn update_bar(mut fill: Mut<Node>, mut text: Mut<Text>, health: &NpcHealth) {
    let ratio = if health.hitpoints_max > 0 {
        health.hitpoints.clamp(0, health.hitpoints_max) as f32 / health.hitpoints_max as f32
    } else {
        0.0
    };
    let width = Val::Percent(ratio * 100.0);
    if fill.width != width {
        fill.width = width;
    }

    let label = format!("{} / {}", health.hitpoints, health.hitpoints_max);
    if text.0 != label {
        text.0 = label;
    }
}
