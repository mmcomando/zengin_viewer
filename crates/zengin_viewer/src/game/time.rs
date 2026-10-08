use bevy::prelude::*;

/// 24 in-game hours pass in 5 minutes of real time.
const HOURS_PER_SECOND: f32 = 24.0 / (5.0 * 60.0);

/// Current time of day of the running game.
#[derive(Resource, Debug)]
pub struct GameTime {
    /// In-game hours in `[0, 24)`, `0.0` is midnight.
    pub total_hours: f32,
    /// Multiplier of the base time speed, `0.0` pauses the time.
    pub speed: f32,
}

impl Default for GameTime {
    fn default() -> Self {
        Self {
            total_hours: 12.0,
            speed: 1.0,
        }
    }
}

impl GameTime {
    pub fn hour(&self) -> u32 {
        (self.total_hours.floor() as u32) % 24
    }
}

fn hour_of(total_hours: f32) -> u32 {
    (total_hours.floor() as u32) % 24
}

fn minute_of(total_hours: f32) -> u32 {
    (total_hours.fract() * 60.0).floor() as u32
}

/// Formats hours in `[0, 24)` as a `HH:MM` clock string.
pub fn clock_string(total_hours: f32) -> String {
    format!("{:02}:{:02}", hour_of(total_hours), minute_of(total_hours))
}

/// Parses `HH:MM` (a missing part defaults to `0`) into hours in `[0, 24)`.
pub fn parse_clock_string(text: &str) -> Option<f32> {
    let (hour, minute) = match text.trim().split_once(':') {
        Some((hour, minute)) => (hour, minute),
        None => (text.trim(), "0"),
    };
    let hour: f32 = hour.trim().parse().ok()?;
    let minute: f32 = minute.trim().parse().ok()?;
    if !(0.0..=23.0).contains(&hour) || !(0.0..=59.0).contains(&minute) {
        return None;
    }
    Some(hour + minute / 60.0)
}

pub(super) fn add_systems(app: &mut App) {
    app.init_resource::<GameTime>();
    app.add_systems(Update, advance_game_time);
}

pub(super) fn advance_game_time(time: Res<Time>, mut game_time: ResMut<GameTime>) {
    game_time.total_hours += time.delta_secs() * HOURS_PER_SECOND * game_time.speed;
    game_time.total_hours %= 24.0;
}
