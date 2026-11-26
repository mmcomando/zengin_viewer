use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::game_state::AppState;
use crate::input::Options;
use crate::zengin::SetGameTime;
use dev_util::perf_debug;

/// Advances the running game time every frame.
pub struct GameTimePlugin;

/// Minutes in a full in-game day.
const MINUTES_PER_DAY: f32 = 24.0 * 60.0;

impl Plugin for GameTimePlugin {
    fn build(&self, app: &mut App) {
        if app.world().resource::<Options>().scenario_enabled() {
            // Repeatable replays: fixed game time, no progression. Inserting
            // before init_resource keeps the preset, init_resource does not
            // overwrite an existing resource.
            app.insert_resource(GameTime {
                total_hours: 12.0,
                speed: 0.0,
                day: 1,
            });
        }
        app.init_resource::<GameTime>();
        app.init_resource::<PlayTime>();
        app.add_systems(
            Update,
            (apply_set_game_time, advance_game_time)
                .chain()
                .run_if(in_state(AppState::InGame)),
        );
        app.add_systems(Update, advance_play_time.run_if(in_state(AppState::InGame)));
    }
}

/// In-game seconds per real second: a full 24-hour day lasts 1 h 36 min of real
/// time.
const GAME_SECONDS_PER_REAL_SECOND: f32 = 15.0;

/// In-game hours per real second.
const HOURS_PER_SECOND: f32 = GAME_SECONDS_PER_REAL_SECOND / (60.0 * 60.0);

/// Current time of day of the running game.
#[derive(Resource, Debug, Clone, Copy, Serialize, Deserialize)]
pub struct GameTime {
    /// In-game hours in `[0, 24)`, `0.0` is midnight.
    pub total_hours: f32,
    /// Multiplier of the base time speed, `0.0` pauses the time.
    pub speed: f32,
    /// In-game day, starting at 1.
    pub day: u32,
}

impl Default for GameTime {
    fn default() -> Self {
        Self {
            total_hours: 12.0,
            speed: 1.0,
            day: 1,
        }
    }
}

impl GameTime {
    pub fn hour(&self) -> u32 {
        (self.total_hours.floor() as u32) % 24
    }

    /// In-game minutes since midnight.
    pub fn minutes_of_day(&self) -> u32 {
        (self.total_hours * 60.0) as u32 % (24 * 60)
    }

    /// In-game minutes since day 1, 00:00, monotonic across day changes.
    pub fn absolute_minutes(&self) -> u32 {
        self.day.saturating_sub(1) * (24 * 60) + self.minutes_of_day()
    }
}

pub(crate) fn advance_game_time(time: Res<Time>, mut game_time: ResMut<GameTime>) {
    let _perf = perf_debug!();
    game_time.total_hours += time.delta_secs() * HOURS_PER_SECOND * game_time.speed;
    let elapsed_days = (game_time.total_hours / 24.0).floor();
    if elapsed_days >= 1.0 {
        game_time.day += elapsed_days as u32;
    }
    game_time.total_hours %= 24.0;
}

/// Applies a script `Wld_SetTime` request (`SetGameTime`).
fn apply_set_game_time(mut requests: MessageReader<SetGameTime>, mut game_time: ResMut<GameTime>) {
    let _perf = perf_debug!();
    for request in requests.read() {
        let now_minutes = game_time.total_hours * 60.0;
        let next_minutes = request.hour as f32 * 60.0 + request.minute as f32;
        let target_minutes = if now_minutes <= next_minutes {
            next_minutes
        } else {
            next_minutes + MINUTES_PER_DAY
        };
        let elapsed_days = (target_minutes / MINUTES_PER_DAY).floor();
        game_time.day += elapsed_days as u32;
        game_time.total_hours = (target_minutes / 60.0) % 24.0;
        info!(
            "Wld_SetTime({:02}:{:02}) -> day {} {:02}:{:02}",
            request.hour,
            request.minute,
            game_time.day,
            game_time.hour(),
            game_time.minutes_of_day() % 60
        );
    }
}

/// Real time spent playing the current save. Accumulates only in `InGame` so
/// menu time is not counted.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct PlayTime {
    pub seconds: f64,
}

impl PlayTime {
    /// Starts a new game's counter from zero.
    pub fn reset(&mut self) {
        self.seconds = 0.0;
    }
}

pub(crate) fn advance_play_time(time: Res<Time>, mut play_time: ResMut<PlayTime>) {
    let _perf = perf_debug!();
    play_time.seconds += time.delta_secs_f64();
}
