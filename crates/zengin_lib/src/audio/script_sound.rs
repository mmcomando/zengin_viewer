//! Sounds requested by the script (`Snd_Play` and friends).

use bevy::prelude::*;

use crate::ScriptSound;
use crate::audio::PlaySound;
use crate::audio::sound_definitions::SoundDefinitions;
use dev_util::perf_debug;

/// Resolves the `C_SFX` definition name to its wav file and requests playback.
pub fn handle_script_sounds(
    mut sounds: MessageReader<ScriptSound>,
    definitions: Res<SoundDefinitions>,
    mut play_sound: MessageWriter<PlaySound>,
) {
    let _perf = perf_debug!();
    for sound in sounds.read() {
        let Some(file) = definitions.file(&sound.name) else {
            warn!("Missing sound definition({})", sound.name);
            continue;
        };
        if file.is_empty() {
            debug!("Sound definition({}) has no file", sound.name);
            continue;
        }
        play_sound.write(PlaySound {
            asset_path: format!("zengin://{file}"),
            position: sound.position,
        });
        debug!(
            "Script sound({}) -> {file}, positional={}",
            sound.name,
            sound.position.is_some()
        );
    }
}
