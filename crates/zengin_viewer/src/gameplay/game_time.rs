use bevy::prelude::*;

use crate::input::Options;

/// Advances the running game time every frame.
pub struct GameTimePlugin;

impl Plugin for GameTimePlugin {
    fn build(&self, app: &mut App) {
        if app.world().resource::<Options>().replay_actions().is_some() {
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
        app.add_systems(Update, advance_game_time);
    }
}

/// 24 in-game hours pass in 5 minutes of real time.
const HOURS_PER_SECOND: f32 = 24.0 / (5.0 * 60.0);

/// Current time of day of the running game.
#[derive(Resource, Debug)]
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
}

pub(crate) fn advance_game_time(time: Res<Time>, mut game_time: ResMut<GameTime>) {
    game_time.total_hours += time.delta_secs() * HOURS_PER_SECOND * game_time.speed;
    let elapsed_days = (game_time.total_hours / 24.0).floor();
    if elapsed_days >= 1.0 {
        game_time.day += elapsed_days as u32;
    }
    game_time.total_hours %= 24.0;
}
