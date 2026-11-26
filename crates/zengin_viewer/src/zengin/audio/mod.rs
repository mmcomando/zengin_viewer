//! Sound and music requests emitted by game code.
//!
//! Gothic-specific resolution (script `C_SFX` names, dialog and SVM voice
//! names, `DirectMusic` themes) lives here; the top-level `audio` module only
//! plays the resulting [`PlaySound`] requests.

mod music;
mod music_definitions;
mod script_sound;
mod sound_definitions;
mod svm;

use bevy::prelude::*;

use crate::zengin::DialogVoice;
use crate::zengin::ScriptChangesApplied;

pub(crate) use crate::zengin::audio::sound_definitions::SoundDefinitions;

/// Resolves Gothic sound requests to asset paths and drives `DirectMusic`
/// playback.
pub(crate) struct ZenginAudioPlugin;

impl Plugin for ZenginAudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<PlaySound>();
        app.add_message::<StopSound>();
        app.init_resource::<PendingMusic>();
        app.init_resource::<svm::SvmPlayback>();
        app.add_plugins(music::MusicPlaybackPlugin);
        app.add_systems(
            Startup,
            (
                sound_definitions::load_sound_definitions,
                music_definitions::load_music_definitions,
            )
                .chain(),
        );
        app.add_systems(
            Update,
            (
                dialog_voice_requests,
                svm::handle_svm_lines,
                script_sound::handle_script_sounds
                    .after(ScriptChangesApplied)
                    .run_if(resource_exists::<sound_definitions::SoundDefinitions>),
            ),
        );
    }
}

/// Asset path of a sound to play.
#[derive(Message)]
pub struct PlaySound {
    pub asset_path: String,
    /// World position of a spatial sound. `None` plays the sound globally, e.g.
    /// a UI cue or a dialog voice.
    pub position: Option<Vec3>,
}

/// Stops the sound started most recently.
#[derive(Message)]
pub struct StopSound;

/// Most recent music request, applied on the next `Update`.
#[derive(Resource, Default)]
pub(crate) struct PendingMusic(Option<MusicRequest>);

pub(crate) enum MusicRequest {
    Play(String),
    Stop,
}

impl PendingMusic {
    pub(crate) fn play(&mut self, name: &str) {
        self.0 = Some(MusicRequest::Play(name.to_string()));
    }

    pub(crate) fn stop(&mut self) {
        self.0 = Some(MusicRequest::Stop);
    }

    pub(crate) fn take(&mut self) -> Option<MusicRequest> {
        self.0.take()
    }
}

fn dialog_voice_requests(
    mut voices: MessageReader<DialogVoice>,
    mut play_sound: MessageWriter<PlaySound>,
    mut stop_sound: MessageWriter<StopSound>,
) {
    for voice in voices.read() {
        match voice {
            DialogVoice::Play { asset_path } => {
                play_sound.write(PlaySound {
                    asset_path: asset_path.clone(),
                    position: None,
                });
            }
            DialogVoice::Stop => {
                stop_sound.write(StopSound);
            }
        }
    }
}
