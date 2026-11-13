//! Smoke tests for the `dmusic` wrapper.
//!
//! The rendering test uses the locally installed Gothic 2 music; it is skipped
//! when the installation cannot be found.

use std::fs;
use std::path::{Path, PathBuf};

use dmusic_sys::{DmEmbellishmentType, DmTiming, Loader, Performance};

#[test]
fn renders_silence_without_a_segment() {
    let performance = Performance::new(44_100).expect("performance");
    let mut buffer = vec![0i16; 1024];
    performance.render_stereo(&mut buffer).expect("render");
    assert!(buffer.iter().all(|sample| *sample == 0));
}

#[test]
fn rejects_odd_stereo_buffers() {
    let performance = Performance::new(44_100).expect("performance");
    let mut buffer = vec![0i16; 3];
    assert!(performance.render_stereo(&mut buffer).is_err());
}

#[test]
fn renders_the_menu_segment() {
    let Some(gothic) = gothic2_dir() else {
        eprintln!("skipping: Gothic 2 installation not found");
        return;
    };
    let dirs = music_dirs(&gothic);

    let mut loader = Loader::new(true).expect("loader");
    loader
        .add_resolver(move |name| resolve_in(&dirs, name))
        .expect("resolver");
    let segment = loader.segment("gamestart.sgt").expect("gamestart.sgt");
    assert!(segment.repeats() > 0, "segment should loop");
    assert!(segment.length() > 0.0, "segment should have a length");

    let performance = Performance::new(44_100).expect("performance");
    performance
        .play_transition(&segment, DmEmbellishmentType::FILL, DmTiming::INSTANT)
        .expect("play");

    // Two seconds of stereo audio, enough to start the first pattern.
    let mut buffer = vec![0i16; 44_100 * 2 * 2];
    performance.render_stereo(&mut buffer).expect("render");

    let nonzero = buffer.iter().filter(|sample| **sample != 0).count();
    assert!(
        nonzero > 1000,
        "expected audible samples from gamestart.sgt, got {nonzero}"
    );
}

fn gothic2_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("GOTHIC2_DIR") {
        return Some(PathBuf::from(dir));
    }
    let home = PathBuf::from(std::env::var_os("HOME")?);
    let path = home.join(".local/share/Steam/steamapps/common/Gothic II");
    path.is_dir().then_some(path)
}

fn music_dirs(gothic: &Path) -> Vec<PathBuf> {
    ["newworld", "AddonWorld", "dungeon", "menu_men", "orchestra"]
        .into_iter()
        .map(|name| gothic.join("_work/Data/Music").join(name))
        .chain(std::iter::once(gothic.join("_work/Data/Music")))
        .collect()
}

fn resolve_in(dirs: &[PathBuf], name: &str) -> Option<Vec<u8>> {
    let name = name.rsplit(['/', '\\']).next().unwrap_or(name);
    for dir in dirs {
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let file_name = entry.file_name();
            if file_name.to_string_lossy().eq_ignore_ascii_case(name) {
                return fs::read(entry.path()).ok();
            }
        }
    }
    None
}
