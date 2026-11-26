mod music;
mod music_definitions;
mod sound_definitions;
mod sound_manager;

use bevy::prelude::*;

use crate::zengin::ScriptChangesApplied;

pub(crate) use crate::audio::music::PendingMusic;
pub(crate) use crate::audio::music_definitions::load_music_definitions;
pub(crate) use crate::audio::sound_definitions::{SoundDefinitions, load_sound_definitions};
pub(crate) use crate::audio::sound_manager::{PlaySound, SoundManager};

/// Plays sounds requested by other systems through the [`PlaySound`] message.
/// Entry point of the audio module, may gather smaller private audio plugins later.
pub struct SoundPlugin;

impl Plugin for SoundPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(music::MusicPlaybackPlugin);
        app.add_message::<PlaySound>();
        app.init_resource::<SoundManager>();
        app.add_systems(
            Startup,
            (load_sound_definitions, load_music_definitions).chain(),
        );
        app.add_systems(
            Update,
            sound_manager::handle_dialog_voices.before(sound_manager::handle_play_sound_messages),
        );
        app.add_systems(Update, sound_manager::handle_play_sound_messages);
        app.add_systems(Update, sound_manager::handle_svm_lines);
        app.add_systems(
            Update,
            sound_manager::handle_script_sounds
                .after(ScriptChangesApplied)
                .run_if(resource_exists::<SoundDefinitions>),
        );
    }
}
