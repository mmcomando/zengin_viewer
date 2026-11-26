use bevy::audio::SpatialScale;
use bevy::prelude::*;
use dev_util::perf_debug;

use zengin_lib::{PlaySound, PlayVoice, StopVoice};

/// Bevy's spatial mixer attenuates with `1/distance²` in world units. The
/// third-person camera sits up to 6 m from the player, which would reduce a
/// voice emitted at the player to a few percent of its volume. Scaling the
/// emitter and listener positions keeps normal gameplay distances at full
/// volume while sounds still fade toward the talk range.
const SPATIAL_SCALE: f32 = 0.15;

/// Entity of the most recently started dialog voice line.
#[derive(Resource, Default)]
pub struct SoundManager {
    active_voice: Option<Entity>,
}

impl SoundManager {
    /// Spawns a voice line and makes it the stoppable one.
    pub fn play_voice(&mut self, commands: &mut Commands, audio_handle: Handle<AudioSource>) {
        self.active_voice = Some(Self::spawn_source(commands, audio_handle, None));
    }

    /// Spawns a fire-and-forget global sound.
    pub fn play_global_sound(commands: &mut Commands, audio_handle: Handle<AudioSource>) {
        Self::spawn_source(commands, audio_handle, None);
    }

    /// Spawns a fire-and-forget sound positioned in the world.
    pub fn play_positional_sound(
        commands: &mut Commands,
        audio_handle: Handle<AudioSource>,
        position: Vec3,
    ) {
        Self::spawn_source(commands, audio_handle, Some(position));
    }

    /// Only the most recently started voice line can be stopped.
    pub fn stop_voice(&mut self, commands: &mut Commands) {
        let Some(entity) = self.active_voice.take() else {
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
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    let _perf = perf_debug!();
    for message in messages.read() {
        let audio_handle = asset_server.load(&message.asset_path);
        match message.position {
            Some(position) => {
                SoundManager::play_positional_sound(&mut commands, audio_handle, position);
            }
            None => SoundManager::play_global_sound(&mut commands, audio_handle),
        }
    }
}

pub(crate) fn handle_play_voice_messages(
    mut messages: MessageReader<PlayVoice>,
    mut sound_manager: ResMut<SoundManager>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    let _perf = perf_debug!();
    for message in messages.read() {
        let audio_handle = asset_server.load(&message.asset_path);
        sound_manager.play_voice(&mut commands, audio_handle);
    }
}

pub(crate) fn handle_stop_voice(
    mut messages: MessageReader<StopVoice>,
    mut sound_manager: ResMut<SoundManager>,
    mut commands: Commands,
) {
    let _perf = perf_debug!();
    for _ in messages.read() {
        sound_manager.stop_voice(&mut commands);
    }
}
