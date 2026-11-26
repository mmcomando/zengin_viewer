//! Plays [`AnimationSound`] messages emitted by the animation playback.

use bevy::prelude::*;

use super::components::AnimationSound;
use crate::audio::{PlaySound, SoundDefinitions};
use crate::gameplay::objects::PlayerCharacter;
use dev_util::perf_debug;

/// New sounds spawned in a single frame, keeps dense scenes from stacking
/// voices.
const MAX_SOUNDS_PER_FRAME: usize = 40;
/// Fallback audible range when the model script leaves it unset, in world
/// units (the engine default is 3500 game units = 35 m).
const DEFAULT_SOUND_RANGE: f32 = 35.0;

#[allow(clippy::cognitive_complexity)]
pub(crate) fn play_animation_sounds(
    mut sounds: MessageReader<AnimationSound>,
    player: Option<Single<&GlobalTransform, With<PlayerCharacter>>>,
    definitions: Option<Res<SoundDefinitions>>,
    mut play_sound: MessageWriter<PlaySound>,
) {
    let _perf = perf_debug!();
    let (Some(player), Some(definitions)) = (player, definitions) else {
        return;
    };
    let player_position = player.translation();
    let mut sounds_played = 0;
    for sound in sounds.read() {
        if sounds_played >= MAX_SOUNDS_PER_FRAME {
            warn!(
                "Too many sounds per frame, skip sound({})",
                sound.sound_name
            );
            continue;
        }
        let range = if sound.range > 0.0 {
            sound.range
        } else {
            DEFAULT_SOUND_RANGE
        };
        if sound.position.distance(player_position) > range {
            continue;
        }
        let Some(file) = definitions.file(&sound.sound_name) else {
            warn!(
                "Missing sound definition({}) for {:?}",
                sound.sound_name, sound.who
            );
            continue;
        };
        play_sound.write(PlaySound {
            asset_path: format!("zengin://{file}"),
            position: Some(sound.position),
        });
        debug!(
            "AnimationSound play: sound={}, file={file}, entity={:?}, distance={:.0}, range={range:.0}",
            sound.sound_name,
            sound.who,
            sound.position.distance(player_position)
        );
        sounds_played += 1;
    }
}
