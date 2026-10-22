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

    /// In-game minutes since midnight.
    pub fn minutes_of_day(&self) -> u32 {
        (self.total_hours * 60.0) as u32 % (24 * 60)
    }
}

pub(crate) fn advance_game_time(time: Res<Time>, mut game_time: ResMut<GameTime>) {
    game_time.total_hours += time.delta_secs() * HOURS_PER_SECOND * game_time.speed;
    game_time.total_hours %= 24.0;
}

// TODO move pub plugin definition to the top of the file
/// Advances the running game time every frame.
pub struct GameTimePlugin;

impl Plugin for GameTimePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameTime>();
        app.add_systems(Update, advance_game_time);
    }
}
