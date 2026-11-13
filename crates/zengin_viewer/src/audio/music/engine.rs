//! `DirectMusic` performance shared by the ECS control path and the audio thread.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use bevy::prelude::*;
use dmusic_sys::{DmEmbellishmentType, DmTiming, Loader, Performance};
use zen_kit_rs::misc::{MusicTransitionEffect, MusicTransitionType};
use zen_kit_rs::music_definitions::MusicTheme;

use crate::zengin::gothic2_dir;

pub(super) const SAMPLE_RATE: u32 = 44_100;
const MUSIC_DIRS: [&str; 5] = ["newworld", "AddonWorld", "dungeon", "menu_men", "orchestra"];

pub(crate) type SharedMusicCore = Arc<Mutex<Option<MusicCore>>>;

/// Playback control of the persistent music performance.
#[allow(dead_code)] // wired into state selection in the next step
#[derive(Resource)]
pub(crate) struct MusicEngine {
    core: SharedMusicCore,
}

#[allow(dead_code)] // wired into state selection in the next step
impl MusicEngine {
    pub fn new() -> Self {
        let core = MusicCore::new();
        if core.is_none() {
            warn!("Music engine unavailable; music will not play");
        }
        Self {
            core: Arc::new(Mutex::new(core)),
        }
    }

    pub fn shared(&self) -> SharedMusicCore {
        Arc::clone(&self.core)
    }

    /// Starts `theme`, reusing the current segment when the file is unchanged.
    pub fn play_theme(&self, name: &str, theme: &MusicTheme) {
        let mut core = lock_core(&self.core);
        let Some(core) = core.as_mut() else {
            return;
        };
        core.play(name, theme);
    }

    pub fn stop(&self) {
        let mut core = lock_core(&self.core);
        let Some(core) = core.as_mut() else {
            return;
        };
        core.stop();
    }

    pub fn set_master_volume(&self, volume: f32) {
        let mut core = lock_core(&self.core);
        let Some(core) = core.as_mut() else {
            return;
        };
        core.master_volume = volume.clamp(0.0, 1.0);
        core.apply_volume();
    }
}

/// Renders interleaved stereo PCM from the shared performance. Called by the
/// audio decoder only.
pub(crate) fn render(shared: &SharedMusicCore, out: &mut [i16]) {
    let mut core = lock_core(shared);
    let Some(core) = core.as_mut() else {
        out.fill(0);
        return;
    };
    if core.performance.render_stereo(out).is_err() {
        out.fill(0);
    }
}

fn lock_core(shared: &SharedMusicCore) -> MutexGuard<'_, Option<MusicCore>> {
    shared.lock().unwrap_or_else(PoisonError::into_inner)
}

pub(crate) struct MusicCore {
    loader: Loader,
    performance: Performance,
    current_file: Option<String>,
    current_volume: f32,
    master_volume: f32,
    missing: HashSet<String>,
}

impl MusicCore {
    fn new() -> Option<Self> {
        let mut loader = match Loader::new(true) {
            Ok(loader) => loader,
            Err(error) => {
                warn!("Failed to create music loader: {error}");
                return None;
            }
        };

        let dirs = music_dirs();
        if let Err(error) = loader.add_resolver(move |name| resolve_music_file(&dirs, name)) {
            warn!("Failed to install music resolver: {error}");
            return None;
        }

        let performance = match Performance::new(SAMPLE_RATE) {
            Ok(performance) => performance,
            Err(error) => {
                warn!("Failed to create music performance: {error}");
                return None;
            }
        };

        Some(Self {
            loader,
            performance,
            current_file: None,
            current_volume: 1.0,
            master_volume: 1.0,
            missing: HashSet::new(),
        })
    }

    #[expect(
        clippy::cognitive_complexity,
        reason = "linear theme-switch flow with explicit failure handling"
    )]
    fn play(&mut self, name: &str, theme: &MusicTheme) {
        if theme.file.is_empty() {
            warn!("Music theme {name} has no segment file, stopping music");
            self.stop();
            return;
        }

        if self
            .current_file
            .as_deref()
            .is_some_and(|file| file.eq_ignore_ascii_case(&theme.file))
        {
            self.current_volume = theme.volume;
            self.apply_volume();
            return;
        }

        let missing_key = theme.file.to_ascii_lowercase();
        if self.missing.contains(&missing_key) {
            return;
        }

        let segment = match self.loader.segment(&theme.file) {
            Ok(segment) => segment,
            Err(error) => {
                warn!(
                    "Music theme {name} segment {} not loaded: {error}",
                    theme.file
                );
                self.missing.insert(missing_key);
                return;
            }
        };

        let embellishment = embellishment(theme.transition);
        let timing = timing(theme.transition_type);

        if let Err(error) = self
            .performance
            .play_transition(&segment, embellishment, timing)
        {
            warn!(
                "Music transition for {name} ({}) failed: {error}; starting it directly",
                theme.file
            );
            if let Err(error) = self.performance.play_segment(Some(&segment), timing) {
                warn!("Music theme {name} could not start: {error}");
                self.current_file = None;
                return;
            }
        }

        self.current_file = Some(theme.file.clone());
        self.current_volume = theme.volume;
        self.apply_volume();
        info!(
            "Music theme {name} -> {} (transition {:?}/{:?})",
            theme.file, theme.transition, theme.transition_type
        );
    }

    fn stop(&mut self) {
        if let Err(error) = self.performance.stop() {
            warn!("Failed to stop music: {error}");
        }
        self.current_file = None;
    }

    fn apply_volume(&self) {
        self.performance
            .set_volume((self.current_volume * self.master_volume).clamp(0.0, 1.0));
    }
}

fn embellishment(effect: MusicTransitionEffect) -> DmEmbellishmentType::Type {
    match effect {
        MusicTransitionEffect::UNKNOWN | MusicTransitionEffect::NONE => DmEmbellishmentType::NONE,
        MusicTransitionEffect::GROOVE => DmEmbellishmentType::GROOVE,
        MusicTransitionEffect::FILL => DmEmbellishmentType::FILL,
        MusicTransitionEffect::BREAK => DmEmbellishmentType::BREAK,
        MusicTransitionEffect::INTRO => DmEmbellishmentType::INTRO,
        MusicTransitionEffect::END => DmEmbellishmentType::END,
        MusicTransitionEffect::END_AND_INTO => DmEmbellishmentType::END_AND_INTRO,
    }
}

fn timing(transition: MusicTransitionType) -> DmTiming::Type {
    match transition {
        MusicTransitionType::UNKNOWN | MusicTransitionType::MEASURE => DmTiming::MEASURE,
        MusicTransitionType::IMMEDIATE => DmTiming::INSTANT,
        MusicTransitionType::BEAT => DmTiming::BEAT,
    }
}

fn music_dirs() -> Vec<PathBuf> {
    let root = Path::new(&gothic2_dir()).join("_work/Data/Music");
    let mut dirs: Vec<PathBuf> = MUSIC_DIRS.into_iter().map(|name| root.join(name)).collect();
    dirs.push(root);
    dirs
}

/// Case-insensitive lookup in the Gothic music directories. dmusic requests
/// file names as written in the segments, while the files on disk are mixed
/// case.
fn resolve_music_file(dirs: &[PathBuf], name: &str) -> Option<Vec<u8>> {
    let name = name.rsplit(['/', '\\']).next().unwrap_or(name);
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let file_name = entry.file_name();
            if file_name.to_string_lossy().eq_ignore_ascii_case(name) {
                return std::fs::read(entry.path()).ok();
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transition_mapping_covers_all_variants() {
        assert_eq!(
            embellishment(MusicTransitionEffect::BREAK),
            DmEmbellishmentType::BREAK
        );
        assert_eq!(
            embellishment(MusicTransitionEffect::END_AND_INTO),
            DmEmbellishmentType::END_AND_INTRO
        );
        assert_eq!(timing(MusicTransitionType::IMMEDIATE), DmTiming::INSTANT);
        assert_eq!(timing(MusicTransitionType::BEAT), DmTiming::BEAT);
        assert_eq!(timing(MusicTransitionType::MEASURE), DmTiming::MEASURE);
    }

    #[test]
    #[ignore = "requires Gothic 2 music and the dmusic sources"]
    fn plays_the_menu_theme() {
        let path = format!(
            "{}/_work/Data/Scripts/_compiled/MUSIC.DAT",
            crate::zengin::gothic2_dir()
        );
        let definitions =
            zen_kit_rs::music_definitions::MusicDefinitions::load_path(&path).expect("MUSIC.DAT");
        let theme = definitions.get("SYS_MENU").expect("SYS_MENU");

        let engine = MusicEngine::new();
        engine.play_theme("SYS_MENU", theme);

        let mut buffer = vec![0i16; SAMPLE_RATE as usize * 2];
        render(&engine.shared(), &mut buffer);
        assert!(buffer.iter().any(|sample| *sample != 0));
    }
}
