mod sound_definitions;
mod sound_manager;

use bevy::prelude::*;

pub(crate) use crate::audio::sound_definitions::{SoundDefinitions, load_sound_definitions};
pub(crate) use crate::audio::sound_manager::{PlaySound, SoundManager, wav_duration};

/// Plays sounds requested by other systems through the [`PlaySound`] message.
/// Entry point of the audio module, may gather smaller private audio plugins later.
pub struct SoundPlugin;

impl Plugin for SoundPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<PlaySound>();
        app.init_resource::<SoundManager>();
        app.add_systems(Startup, load_sound_definitions);
        app.add_systems(Update, sound_manager::handle_play_sound_messages);
    }
}
