//! Persistent `DirectMusic` playback.
//!
//! One [`DmPerformance`](dmusic_sys::Performance) lives for the whole
//! application so theme changes can use `DirectMusic` transitions. A single
//! long-lived Bevy audio entity renders that performance through
//! [`MusicSource`].

mod engine;
mod source;

use bevy::audio::AddAudioSource;
use bevy::prelude::*;

use crate::audio::music::engine::MusicEngine;
use crate::audio::music::source::MusicSource;

/// Registers the music audio source and spawns the single playback entity.
pub(crate) struct MusicPlaybackPlugin;

impl Plugin for MusicPlaybackPlugin {
    fn build(&self, app: &mut App) {
        app.add_audio_source::<MusicSource>();
        app.add_systems(Startup, spawn_music_player);
    }
}

fn spawn_music_player(mut commands: Commands, mut sources: ResMut<Assets<MusicSource>>) {
    let engine = MusicEngine::new();
    let source = sources.add(MusicSource::new(engine.shared()));
    // The decoder is an infinite source; `ONCE` just keeps the same sink alive.
    commands.spawn((AudioPlayer(source), PlaybackSettings::ONCE));
    commands.insert_resource(engine);
}
