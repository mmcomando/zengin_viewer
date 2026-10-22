// TODO move this to dialog submodule
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use zen_kit_rs::vfs::Vfs;

use crate::audio::wav_duration;

/// Durations of speech wav files, read directly from the shared vfs and
/// cached, keyed by the zengin asset path of the sound
#[derive(Resource)]
pub struct SpeechSounds {
    vfs: Arc<Mutex<Vfs>>,
    durations: Mutex<HashMap<String, Option<f32>>>,
}

impl SpeechSounds {
    pub fn new(vfs: Arc<Mutex<Vfs>>) -> Self {
        Self {
            vfs,
            durations: Mutex::new(HashMap::new()),
        }
    }

    /// Duration in seconds of the speech sound with the given zengin asset path
    pub fn duration(&self, asset_path: &str) -> Option<f32> {
        let mut durations = self.durations.lock().unwrap();
        if let Some(duration) = durations.get(asset_path) {
            return *duration;
        }

        let duration = self.read_duration(asset_path);
        durations.insert(asset_path.to_string(), duration);
        duration
    }

    fn read_duration(&self, asset_path: &str) -> Option<f32> {
        let relative_path = asset_path.strip_prefix("zengin://")?;
        let vfs = self.vfs.lock().unwrap();
        let bytes = super::loaders::vdf_reader::get_zengin_asset_bytes(&vfs, relative_path)?;
        wav_duration(&bytes)
    }
}
