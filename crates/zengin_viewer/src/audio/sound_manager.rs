use std::collections::HashMap;

use bevy::audio::SpatialScale;
use bevy::prelude::*;

use crate::audio::SoundDefinitions;
use crate::gameplay::objects::PlayerCharacter;
use crate::zengin::InstanceId;
use crate::zengin::ScriptSound;
use crate::zengin::SpeechSounds;
use crate::zengin::SvmLine;
use crate::zengin::get_dialog_sound_path;

/// `WorldSound::talkRange` in engine units (2000) expressed in meters: speech
/// outside this distance from the listener is not played.
const SVM_TALK_RANGE: f32 = 20.0;

/// Bevy's spatial mixer attenuates with `1/distance²` in world units. The
/// third-person camera sits up to 6 m from the player, which would reduce a
/// voice emitted at the player to a few percent of its volume. Scaling the
/// emitter and listener positions keeps normal gameplay distances at full
/// volume while sounds still fade toward the talk range.
const SPATIAL_SCALE: f32 = 0.15;

/// Asset path of the sound which should be played
#[derive(Message)]
pub struct PlaySound {
    pub asset_path: String,
    /// World position of a spatial sound. `None` plays the sound globally, e.g.
    /// a UI cue or a dialog voice.
    pub position: Option<Vec3>,
}

/// Plays sounds requested by other systems
#[derive(Resource, Default)]
pub struct SoundManager {
    /// Entity of the most recently spawned sound
    active: Option<Entity>,
    /// World time in seconds until which `AI_OutputSVM_Overlay` lines of each
    /// speaker are suppressed because that speaker's previous overlay line is
    /// still playing.
    svm_barriers: HashMap<InstanceId, f64>,
}

impl SoundManager {
    /// Spawns a fire-and-forget entity playing the given audio source
    pub fn play_sound(&mut self, commands: &mut Commands, audio_handle: Handle<AudioSource>) {
        let entity = Self::spawn_source(commands, audio_handle, None);
        self.active = Some(entity);
    }

    /// Spawns a fire-and-forget global sound. It is not a dialog voice, so it
    /// must not replace [`SoundManager::active`].
    pub fn play_global_sound(commands: &mut Commands, audio_handle: Handle<AudioSource>) {
        Self::spawn_source(commands, audio_handle, None);
    }

    /// Spawns a fire-and-forget sound positioned in the world. Like
    /// [`SoundManager::play_global_sound`] it is not tracked as the active
    /// dialog voice.
    pub fn play_positional_sound(
        commands: &mut Commands,
        audio_handle: Handle<AudioSource>,
        position: Vec3,
    ) {
        Self::spawn_source(commands, audio_handle, Some(position));
    }

    /// Only the most recently spawned sound can be stopped
    pub fn stop_active(&mut self, commands: &mut Commands) {
        let Some(entity) = self.active.take() else {
            return;
        };
        commands.entity(entity).try_despawn();
    }

    fn spawn_source(
        commands: &mut Commands,
        audio_handle: Handle<AudioSource>,
        position: Option<Vec3>,
    ) -> Entity {
        let mut settings = PlaybackSettings::DESPAWN.with_spatial(position.is_some());
        if position.is_some() {
            settings = settings.with_spatial_scale(SpatialScale::new(SPATIAL_SCALE));
        }
        let mut entity = commands.spawn((AudioPlayer::new(audio_handle), settings));
        if let Some(position) = position {
            entity.insert(Transform::from_translation(position));
        }
        entity.id()
    }
}

pub(crate) fn handle_play_sound_messages(
    mut messages: MessageReader<PlaySound>,
    mut sound_manager: ResMut<SoundManager>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    for message in messages.read() {
        let audio_handle = asset_server.load(&message.asset_path);
        match message.position {
            Some(position) => {
                SoundManager::play_positional_sound(&mut commands, audio_handle, position);
            }
            None => sound_manager.play_sound(&mut commands, audio_handle),
        }
    }
}

/// Plays the voice lines requested by `AI_OutputSVM`/`AI_OutputSVM_Overlay`,
/// positional at the speaking npc. Overlay lines are suppressed while the same
/// speaker's previous overlay line plays, and lines outside the listener range
/// are skipped (the barrier is still set, matching the engine).
pub(crate) fn handle_svm_lines(
    time: Res<Time>,
    mut lines: MessageReader<SvmLine>,
    asset_server: Res<AssetServer>,
    speech_sounds: Option<Res<SpeechSounds>>,
    players: Query<&GlobalTransform, With<PlayerCharacter>>,
    mut sound_manager: ResMut<SoundManager>,
    mut commands: Commands,
) {
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
            &mut sound_manager,
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
        let audio_handle = asset_server.load(&path);
        match line.position {
            Some(position) => {
                SoundManager::play_positional_sound(&mut commands, audio_handle, position);
            }
            None => SoundManager::play_global_sound(&mut commands, audio_handle),
        }
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
    sound_manager: &mut SoundManager,
) -> bool {
    if !line.overlay {
        return true;
    }
    if sound_manager
        .svm_barriers
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
    sound_manager
        .svm_barriers
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

/// Plays the sounds requested by the script (`Snd_Play` and friends),
/// resolving the `C_SFX` definition name to its wav file.
pub(crate) fn handle_script_sounds(
    mut sounds: MessageReader<ScriptSound>,
    definitions: Res<SoundDefinitions>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    for sound in sounds.read() {
        let Some(file) = definitions.file(&sound.name) else {
            warn!("Missing sound definition({})", sound.name);
            continue;
        };
        if file.is_empty() {
            debug!("Sound definition({}) has no file", sound.name);
            continue;
        }
        let audio_handle = asset_server.load(format!("zengin://{file}"));
        match sound.position {
            Some(position) => {
                SoundManager::play_positional_sound(&mut commands, audio_handle, position);
            }
            None => SoundManager::play_global_sound(&mut commands, audio_handle),
        }
        debug!(
            "Script sound({}) -> {file}, positional={}",
            sound.name,
            sound.position.is_some()
        );
    }
}

/// Duration in seconds of the audio data in a wav file, None if not parseable
pub fn wav_duration(bytes: &[u8]) -> Option<f32> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return None;
    }

    let mut offset = 12usize;
    let mut sample_rate = None;
    let mut byte_rate = None;
    let mut samples = None;
    let mut data_size = None;

    while offset + 8 <= bytes.len() {
        let id = &bytes[offset..offset + 4];
        let size = u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().ok()?) as usize;
        let payload = offset + 8;
        if payload + size > bytes.len() {
            return None;
        }

        match id {
            b"fmt " if size >= 16 => {
                sample_rate = Some(u32::from_le_bytes(
                    bytes[payload + 4..payload + 8].try_into().ok()?,
                ));
                byte_rate = Some(u32::from_le_bytes(
                    bytes[payload + 8..payload + 12].try_into().ok()?,
                ));
            }
            b"fact" if size >= 4 => {
                samples = Some(u32::from_le_bytes(
                    bytes[payload..payload + 4].try_into().ok()?,
                ));
            }
            b"data" => {
                data_size = Some(size);
            }
            _ => {}
        }

        // Chunks are word aligned, odd sized chunks have a padding byte
        offset = payload + size + (size & 1);
    }

    if let (Some(samples), Some(sample_rate)) = (samples, sample_rate)
        && sample_rate > 0
    {
        return Some(samples as f32 / sample_rate as f32);
    }
    if let (Some(data_size), Some(byte_rate)) = (data_size, byte_rate)
        && byte_rate > 0
    {
        return Some(data_size as f32 / byte_rate as f32);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a minimal wav byte buffer: fmt chunk (pcm 16 bit mono),
    /// optional fact chunk and a data chunk
    fn wav_bytes(
        sample_rate: u32,
        byte_rate: u32,
        samples: Option<u32>,
        data_size: u32,
    ) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(b"WAVE");

        bytes.extend_from_slice(b"fmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&sample_rate.to_le_bytes());
        bytes.extend_from_slice(&byte_rate.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());

        if let Some(samples) = samples {
            bytes.extend_from_slice(b"fact");
            bytes.extend_from_slice(&4u32.to_le_bytes());
            bytes.extend_from_slice(&samples.to_le_bytes());
        }

        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_size.to_le_bytes());
        bytes.extend(std::iter::repeat_n(0u8, data_size as usize));
        bytes
    }

    #[test]
    fn duration_from_fact_chunk() {
        let bytes = wav_bytes(44_100, 22_125, Some(240_690), 120_832);
        let duration = wav_duration(&bytes).unwrap();
        assert!((duration - 5.457_823).abs() < 1e-4);
    }

    #[test]
    fn duration_falls_back_to_data_chunk() {
        let bytes = wav_bytes(8_000, 16_000, None, 8_000);
        let duration = wav_duration(&bytes).unwrap();
        assert!((duration - 0.5).abs() < 1e-4);
    }

    #[test]
    fn garbage_bytes_are_rejected() {
        assert_eq!(wav_duration(&[]), None);
        assert_eq!(wav_duration(b"not a wav file at all...."), None);
    }

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
        let mut manager = SoundManager::default();
        let overlay = line(true);

        assert!(overlay_allowed(&overlay, "path", 1.0, None, &mut manager));
        assert!(!overlay_allowed(&overlay, "path", 0.5, None, &mut manager));
        assert!(overlay_allowed(&overlay, "path", 1.0, None, &mut manager));
    }

    #[test]
    fn overlay_line_of_another_speaker_is_not_blocked() {
        let mut manager = SoundManager::default();
        let first = line(true);
        let mut second = line(true);
        second.speaker = InstanceId::new(2).unwrap();

        assert!(overlay_allowed(&first, "path", 1.0, None, &mut manager));
        assert!(overlay_allowed(&second, "path", 0.5, None, &mut manager));
    }

    #[test]
    fn regular_line_does_not_set_the_overlay_barrier() {
        let mut manager = SoundManager::default();

        assert!(overlay_allowed(
            &line(false),
            "path",
            1.0,
            None,
            &mut manager
        ));
        assert!(manager.svm_barriers.is_empty());
    }

    #[test]
    fn listener_gate_skips_only_distant_positional_lines() {
        let listener = Vec3::ZERO;
        assert!(in_listener_range(
            Some(listener),
            Some(Vec3::new(5.0, 0.0, 0.0))
        ));
        assert!(!in_listener_range(
            Some(listener),
            Some(Vec3::new(25.0, 0.0, 0.0))
        ));
        assert!(in_listener_range(Some(listener), None));
        assert!(in_listener_range(None, Some(Vec3::new(25.0, 0.0, 0.0))));
    }
}
