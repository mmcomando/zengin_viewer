use bevy::prelude::*;

/// Asset path of the sound which should be played
#[derive(Message)]
pub struct PlaySound {
    pub asset_path: String,
}

/// Plays sounds requested by other systems
#[derive(Resource, Default)]
pub struct SoundManager;

impl SoundManager {
    /// Spawns a fire-and-forget entity playing the given audio source
    pub fn play_sound(&mut self, commands: &mut Commands, audio_handle: Handle<AudioSource>) {
        commands.spawn((AudioPlayer::new(audio_handle), PlaybackSettings::DESPAWN));
    }
}

pub struct GameAudioPlugin;

impl Plugin for GameAudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<PlaySound>();
        app.init_resource::<SoundManager>();
        app.add_systems(Update, handle_play_sound_messages);
    }
}

fn handle_play_sound_messages(
    mut messages: MessageReader<PlaySound>,
    mut sound_manager: ResMut<SoundManager>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    for message in messages.read() {
        let audio_handle = asset_server.load(&message.asset_path);
        sound_manager.play_sound(&mut commands, audio_handle);
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
}
