use bevy::audio::SpatialScale;
use bevy::prelude::*;
use dev_util::perf_debug;

use crate::zengin::{PlaySound, StopSound};

/// Bevy's spatial mixer attenuates with `1/distance²` in world units. The
/// third-person camera sits up to 6 m from the player, which would reduce a
/// voice emitted at the player to a few percent of its volume. Scaling the
/// emitter and listener positions keeps normal gameplay distances at full
/// volume while sounds still fade toward the talk range.
const SPATIAL_SCALE: f32 = 0.15;

/// Entity of the most recently spawned sound.
#[derive(Resource, Default)]
pub struct SoundManager {
    active: Option<Entity>,
}

impl SoundManager {
    /// Spawns a fire-and-forget entity playing the given audio source
    pub fn play_sound(&mut self, commands: &mut Commands, audio_handle: Handle<AudioSource>) {
        let entity = Self::spawn_source(commands, audio_handle, None);
        self.active = Some(entity);
    }

    /// Spawns a fire-and-forget sound positioned in the world. It is not tracked
    /// by [`SoundManager::stop_active`].
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
    let _perf = perf_debug!();
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

pub(crate) fn handle_stop_sound(
    mut messages: MessageReader<StopSound>,
    mut sound_manager: ResMut<SoundManager>,
    mut commands: Commands,
) {
    let _perf = perf_debug!();
    for _ in messages.read() {
        sound_manager.stop_active(&mut commands);
    }
}
