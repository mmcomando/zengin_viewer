//! Music themes from the compiled `MUSIC.DAT` script.

use bevy::prelude::*;
use zen_kit_rs::music_definitions::{MusicDefinitions as ZkMusicDefinitions, MusicTheme};

use crate::perf_debug;
use crate::zengin::{ZenginVfs, gothic2_dir};

/// Music theme instance name to its segment and playback settings.
#[derive(Resource, Default)]
pub(crate) struct MusicDefinitions {
    inner: ZkMusicDefinitions,
}

impl MusicDefinitions {
    /// Theme by instance name (`SYS_Menu`, `KHO_DAY_STD`, ...), case-insensitive.
    pub fn theme(&self, name: &str) -> Option<&MusicTheme> {
        self.inner.get(name)
    }
}

pub(crate) fn load_music_definitions(mut commands: Commands, vfs: Res<ZenginVfs>) {
    let _perf = perf_debug!();
    let loose_path = format!("{}/_work/Data/Scripts/_compiled/MUSIC.DAT", gothic2_dir());
    let loaded = ZkMusicDefinitions::load_path(&loose_path).or_else(|| {
        let vfs = vfs.0.lock().unwrap();
        ZkMusicDefinitions::load_vfs(&vfs, "MUSIC.DAT")
    });

    let Some(inner) = loaded else {
        warn!("Music definitions(MUSIC.DAT) not loaded, music will not play");
        commands.insert_resource(MusicDefinitions::default());
        return;
    };

    info!("Loaded {} music themes", inner.len());
    commands.insert_resource(MusicDefinitions { inner });
}
