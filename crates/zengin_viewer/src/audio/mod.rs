//! Plays sound assets requested through [`PlaySound`](crate::zengin::PlaySound).

mod sound_manager;

use bevy::prelude::*;

pub(crate) use crate::audio::sound_manager::SoundManager;

/// Plays sounds requested by game code through the
/// [`PlaySound`](crate::zengin::PlaySound) message.
pub struct SoundPlugin;

impl Plugin for SoundPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SoundManager>();
        app.add_systems(
            Update,
            (
                sound_manager::handle_stop_sound,
                sound_manager::handle_play_sound_messages,
            )
                .chain(),
        );
    }
}
