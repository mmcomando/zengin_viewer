//! Bottom-left log of recent game messages, each entry shown for a few seconds.

use bevy::prelude::*;

use crate::gui::{JournalUiState, StatsUiState};
use crate::perf_debug;
use crate::ui_kit::set_visibility;

pub struct GameLogPlugin;

impl Plugin for GameLogPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameLog>();
        app.add_message::<GameLogEntry>();
        app.add_systems(Startup, spawn_game_log);
        app.add_systems(
            Update,
            (
                handle_game_log_messages,
                update_game_log,
                update_game_log_visibility,
            )
                .chain(),
        );
    }
}

/// How long a single entry stays visible.
const ENTRY_LIFETIME: f32 = 0.5;
/// Maximum number of entries kept, oldest are dropped first.
const MAX_ENTRIES: usize = 8;
/// Font size of a log line in logical pixels.
const LOG_FONT_SIZE: f32 = 16.0;

/// Recently logged messages, each with its remaining display time.
#[derive(Resource, Default)]
pub struct GameLog {
    entries: Vec<LogEntry>,
}

struct LogEntry {
    text: String,
    time_left: f32,
}

impl GameLog {
    /// Adds an entry, shown for [`ENTRY_LIFETIME`] seconds.
    pub fn add(&mut self, text: impl Into<String>) {
        self.entries.push(LogEntry {
            text: text.into(),
            time_left: ENTRY_LIFETIME,
        });
        if self.entries.len() > MAX_ENTRIES {
            self.entries.remove(0);
        }
    }
}

/// Requests a new [`GameLog`] entry, written by systems that cannot (or should
/// not) access the log resource directly.
#[derive(Message)]
pub struct GameLogEntry {
    pub text: String,
}

fn handle_game_log_messages(mut messages: MessageReader<GameLogEntry>, mut log: ResMut<GameLog>) {
    let _perf = perf_debug!();
    for message in messages.read() {
        log.add(message.text.as_str());
    }
}

/// Marks the game log text node which content is replaced every frame.
#[derive(Component)]
struct GameLogTextMarker;

fn spawn_game_log(mut commands: Commands) {
    let _perf = perf_debug!();
    commands.spawn((
        GameLogTextMarker,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(12.0),
            bottom: Val::Px(36.0),
            ..default()
        },
        Text::new(""),
        TextFont {
            font_size: LOG_FONT_SIZE.into(),
            ..default()
        },
        TextColor(Color::srgb(0.9, 0.9, 0.9)),
        TextShadow {
            offset: Vec2::new(1.0, 1.0),
            color: Color::BLACK,
        },
    ));
}

// TODO Maybe it is possible to run this only on change of GameLog? and time elapse? not in every frame?
// TODO Revisit when Resources will be Components in bevy > 0.20.
fn update_game_log(
    time: Res<Time>,
    mut log: ResMut<GameLog>,
    mut text: Query<&mut Text, With<GameLogTextMarker>>,
) {
    let _perf = perf_debug!();
    let delta = time.delta_secs();
    for entry in &mut log.entries {
        entry.time_left -= delta;
    }
    log.entries.retain(|entry| entry.time_left > 0.0);

    let Ok(mut text) = text.single_mut() else {
        return;
    };
    let joined = log
        .entries
        .iter()
        .map(|entry| entry.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    if text.0 != joined {
        text.0 = joined;
    }
}

/// The menu screens are a smaller area than the window, so the log would show
/// next to them.
fn update_game_log_visibility(
    journal: Res<JournalUiState>,
    stats: Res<StatsUiState>,
    mut texts: Query<&mut Visibility, With<GameLogTextMarker>>,
) {
    let _perf = perf_debug!();
    let visible = !journal.is_open() && !stats.is_open();
    for mut visibility in &mut texts {
        set_visibility(&mut visibility, visible);
    }
}
