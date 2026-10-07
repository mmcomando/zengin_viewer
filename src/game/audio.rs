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
