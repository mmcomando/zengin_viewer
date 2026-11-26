//! Voice lines requested by `AI_OutputSVM` / `AI_OutputSVM_Overlay`.

use std::collections::HashMap;

use bevy::prelude::*;

use crate::audio::PlaySound;
use crate::get_dialog_sound_path;
use crate::{InstanceId, PlayerCharacter, SpeechSounds, SvmLine};
use dev_util::perf_debug;

/// `WorldSound::talkRange` in engine units (2000) expressed in meters: speech
/// outside this distance from the listener is not played.
const SVM_TALK_RANGE: f32 = 20.0;

/// Per-speaker suppression windows for overlay voice lines, in seconds.
#[derive(Resource, Default)]
pub struct SvmPlayback {
    barriers: HashMap<InstanceId, f64>,
}

/// Plays the voice lines requested by `AI_OutputSVM`/`AI_OutputSVM_Overlay`,
/// positional at the speaking npc. Overlay lines are suppressed while the same
/// speaker's previous overlay line plays, and lines outside the listener range
/// are skipped (the barrier is still set, matching the engine).
pub fn handle_svm_lines(
    time: Res<Time>,
    mut lines: MessageReader<SvmLine>,
    mut play_sound: MessageWriter<PlaySound>,
    speech_sounds: Option<Res<SpeechSounds>>,
    players: Query<&GlobalTransform, With<PlayerCharacter>>,
    mut playback: ResMut<SvmPlayback>,
) {
    let _perf = perf_debug!();
    let listener = players
        .iter()
        .next()
        .map(|transform| transform.translation());
    for line in lines.read() {
        let path = get_dialog_sound_path(&line.sound);
        if !overlay_allowed(
            line,
            &path,
            time.elapsed_secs_f64(),
            speech_sounds.as_deref(),
            &mut playback,
        ) {
            continue;
        }
        if !in_listener_range(listener, line.position) {
            debug!(
                "Script svm({}) skipped, outside the listener range ({SVM_TALK_RANGE} m)",
                line.sound
            );
            continue;
        }
        play_sound.write(PlaySound {
            asset_path: path.clone(),
            position: line.position,
        });
        debug!(
            "Script svm({}) -> {path}, positional={}",
            line.sound,
            line.position.is_some()
        );
    }
}

/// Overlay lines are suppressed while the same speaker's previous overlay line
/// plays. The barrier is set from the sound duration even when the line is
/// later skipped for being out of range, matching the engine.
fn overlay_allowed(
    line: &SvmLine,
    path: &str,
    now: f64,
    speech_sounds: Option<&SpeechSounds>,
    playback: &mut SvmPlayback,
) -> bool {
    if !line.overlay {
        return true;
    }
    if playback
        .barriers
        .get(&line.speaker)
        .is_some_and(|until| now < *until)
    {
        debug!(
            "Script svm overlay({}) skipped, the speaker's previous line still plays",
            line.sound
        );
        return false;
    }
    let duration = speech_sounds
        .and_then(|speech| speech.duration(path))
        .unwrap_or(0.0);
    playback
        .barriers
        .insert(line.speaker, now + f64::from(duration));
    true
}

/// A line without a position is global, and a missing listener disables the
/// range gate (e.g. before the player exists).
fn in_listener_range(listener: Option<Vec3>, position: Option<Vec3>) -> bool {
    match (listener, position) {
        (Some(listener), Some(position)) => listener.distance(position) <= SVM_TALK_RANGE,
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(overlay: bool) -> SvmLine {
        SvmLine {
            speaker: InstanceId::new(1).unwrap(),
            sound: "SVM_1_Test".to_string(),
            position: Some(Vec3::ZERO),
            overlay,
        }
    }

    #[test]
    fn overlay_line_blocks_the_next_one_until_the_barrier() {
        let mut playback = SvmPlayback::default();
        let overlay = line(true);

        assert!(overlay_allowed(&overlay, "path", 1.0, None, &mut playback));
        assert!(!overlay_allowed(&overlay, "path", 0.5, None, &mut playback));
        assert!(overlay_allowed(&overlay, "path", 1.0, None, &mut playback));
    }

    #[test]
    fn overlay_line_of_another_speaker_is_not_blocked() {
        let mut playback = SvmPlayback::default();
        let first = line(true);
        let mut second = line(true);
        second.speaker = InstanceId::new(2).unwrap();

        assert!(overlay_allowed(&first, "path", 1.0, None, &mut playback));
        assert!(overlay_allowed(&second, "path", 0.5, None, &mut playback));
    }

    #[test]
    fn regular_line_does_not_set_the_overlay_barrier() {
        let mut playback = SvmPlayback::default();

        assert!(overlay_allowed(
            &line(false),
            "path",
            1.0,
            None,
            &mut playback
        ));
        assert!(playback.barriers.is_empty());
    }
}
