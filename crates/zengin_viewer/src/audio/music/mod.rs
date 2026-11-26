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
use crate::audio::music_definitions::MusicDefinitions;
use crate::zengin::AppState;
use dev_util::perf_debug;

/// Latest music request from a menu or gameplay system. It is applied in
/// `Update`, after the startup systems had a chance to create the engine and
/// load `MUSIC.DAT`.
#[derive(Resource, Default)]
pub(crate) struct PendingMusic(Option<MusicRequest>);

enum MusicRequest {
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
}

/// Registers the music audio source and spawns the single playback entity.
pub(crate) struct MusicPlaybackPlugin;

impl Plugin for MusicPlaybackPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PendingMusic>();
        app.add_audio_source::<MusicSource>();
        app.add_systems(Startup, spawn_music_player);
        app.add_systems(Update, apply_pending_music);
        app.add_systems(OnEnter(AppState::LoadScreen), request_loading_theme);
        app.add_systems(OnExit(AppState::LoadScreen), request_stop);
    }
}

fn spawn_music_player(mut commands: Commands, mut sources: ResMut<Assets<MusicSource>>) {
    let _perf = perf_debug!();
    let engine = MusicEngine::new();
    let source = sources.add(MusicSource::new(engine.shared()));
    // The decoder is an infinite source; `ONCE` just keeps the same sink alive.
    commands.spawn((AudioPlayer(source), PlaybackSettings::ONCE));
    commands.insert_resource(engine);
}

fn apply_pending_music(
    mut pending: ResMut<PendingMusic>,
    engine: Option<Res<MusicEngine>>,
    definitions: Option<Res<MusicDefinitions>>,
) {
    let _perf = perf_debug!();
    let (Some(engine), Some(definitions)) = (engine, definitions) else {
        return;
    };
    let Some(request) = pending.0.take() else {
        return;
    };
    match request {
        MusicRequest::Play(name) => engine.play_named(&name, &definitions),
        MusicRequest::Stop => engine.stop(),
    }
}

fn request_loading_theme(mut pending: ResMut<PendingMusic>) {
    let _perf = perf_debug!();
    pending.play("SYS_Loading");
}

fn request_stop(mut pending: ResMut<PendingMusic>) {
    let _perf = perf_debug!();
    pending.stop();
}
