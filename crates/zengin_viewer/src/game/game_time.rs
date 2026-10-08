use bevy::prelude::*;

use super::{lighting, npc_routines, time};

/// Wires game time, day/night lighting and npc routine teleporting into the app.
pub struct GameTimePlugin;

impl Plugin for GameTimePlugin {
    fn build(&self, app: &mut App) {
        time::add_systems(app);
        lighting::add_systems(app);
        npc_routines::add_systems(app);
    }
}
