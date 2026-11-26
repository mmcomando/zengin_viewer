//! Plays sound assets requested through [`PlaySound`] and dialog voice lines
//! requested through [`PlayVoice`]/[`StopVoice`](zengin_lib::StopVoice).

mod sound_manager;

use bevy::prelude::*;

pub(crate) use crate::audio::sound_manager::SoundManager;

/// Plays sounds and dialog voice lines requested by game code.
pub struct SoundPlugin;

impl Plugin for SoundPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SoundManager>();
        app.add_systems(
            Update,
            (
                sound_manager::handle_stop_voice,
                sound_manager::handle_play_voice_messages,
                sound_manager::handle_play_sound_messages,
            )
                .chain(),
        );
    }
}
