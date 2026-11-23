//! Sound definitions from the compiled `SFX.DAT` script.

use bevy::prelude::*;
use zen_kit_rs::sound_definitions::SoundDefinitions as ZkSoundDefinitions;

use crate::perf_debug;
use crate::zengin::{ZenginVfs, gothic2_dir};

/// Sound definition name (e.g. `WAR_AMBIENT`) to wav file name.
#[derive(Resource, Default)]
pub struct SoundDefinitions {
    inner: Option<ZkSoundDefinitions>,
}

impl SoundDefinitions {
    /// Wav file name to play for the sound definition, None when the script
    /// does not define it.
    pub fn file(&self, name: &str) -> Option<&str> {
        self.inner.as_ref()?.file(name)
    }
}

pub(crate) fn load_sound_definitions(mut commands: Commands, vfs: Res<ZenginVfs>) {
    let _perf = perf_debug!();
    let loose_path = format!("{}/_work/Data/Scripts/_compiled/SFX.DAT", gothic2_dir());
    let loaded = ZkSoundDefinitions::load_path(&loose_path).or_else(|| {
        let vfs = vfs.0.lock().unwrap();
        ZkSoundDefinitions::load_vfs(&vfs, "SFX.DAT")
    });

    let Some(inner) = loaded else {
        warn!("Sound definitions(SFX.DAT) not loaded, animation sounds will not play");
        commands.insert_resource(SoundDefinitions::default());
        return;
    };

    info!("Loaded {} sound definitions", inner.len());
    commands.insert_resource(SoundDefinitions { inner: Some(inner) });
}
