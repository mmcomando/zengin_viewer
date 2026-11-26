//! Read, write and enumerate timestamped save directories.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use bevy::log::warn;
use serde::Serialize;
use serde::de::DeserializeOwned;

use super::format::{
    GLOBALS_FILE, GUILDS_FILE, GlobalsSave, GuildsSave, INSTANCES_FILE, ITEMS_FILE, InstancesSave,
    ItemsSave, JOURNAL_FILE, JournalSave, METADATA_FILE, NPCS_FILE, NpcsSave, SaveGame,
    SaveMetadata, ScriptSnapshot, WORLD_FILE, WorldSave, format_unix_utc, saves_dir,
};
use dev_util::hash::fnv1a_64;

use crate::zengin::gothic_dat_path;

/// Prefix of every save directory this module manages. Other files in the save
/// directory (for example the legacy `quicksave.ron`) are ignored.
const SAVE_PREFIX: &str = "save_";

/// Prefix of the temporary directory a save is built in before it is renamed.
const TMP_PREFIX: &str = ".tmp-";

/// One listed save: its directory and the metadata needed by the load screen.
#[derive(Debug, Clone)]
pub(crate) struct SaveEntry {
    pub(crate) path: PathBuf,
    pub(crate) metadata: SaveMetadata,
}

/// Writes a save directory atomically: every document goes into a temporary
/// sibling directory that is renamed to the final name only after all writes
/// succeeded.
pub(crate) fn write_save(save: &SaveGame, dir: &Path) -> Result<(), String> {
    let Some(parent) = dir.parent() else {
        return Err(format!(
            "save path({}) has no parent directory",
            dir.display()
        ));
    };
    let Some(name) = dir.file_name() else {
        return Err(format!("save path({}) has no file name", dir.display()));
    };
    if dir.exists() {
        return Err(format!("save directory {} already exists", dir.display()));
    }
    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "failed to create save directory {}: {error}",
            parent.display()
        )
    })?;

    let tmp_dir = parent.join(format!("{TMP_PREFIX}{}", name.to_string_lossy()));
    if tmp_dir.exists() {
        fs::remove_dir_all(&tmp_dir)
            .map_err(|error| format!("failed to clear {}: {error}", tmp_dir.display()))?;
    }

    if let Err(error) = write_save_files(save, &tmp_dir) {
        if let Err(cleanup_error) = fs::remove_dir_all(&tmp_dir) {
            warn!(
                "Failed to remove incomplete save {}: {cleanup_error}",
                tmp_dir.display()
            );
        }
        return Err(error);
    }
    fs::rename(&tmp_dir, dir).map_err(|error| {
        format!(
            "failed to move {} to {}: {error}",
            tmp_dir.display(),
            dir.display()
        )
    })?;
    Ok(())
}

fn write_save_files(save: &SaveGame, dir: &Path) -> Result<(), String> {
    fs::create_dir_all(dir)
        .map_err(|error| format!("failed to create save directory {}: {error}", dir.display()))?;

    write_ron(&dir.join(METADATA_FILE), &save.metadata)?;
    write_ron(
        &dir.join(WORLD_FILE),
        &WorldSave {
            objects: save.objects.clone(),
            mob_routines: save.script.mob_routines.clone(),
        },
    )?;
    write_ron(
        &dir.join(NPCS_FILE),
        &NpcsSave {
            npc_runtimes: save.script.npc_runtimes.clone(),
        },
    )?;
    write_ron(
        &dir.join(ITEMS_FILE),
        &ItemsSave {
            item_runtimes: save.script.item_runtimes.clone(),
        },
    )?;
    write_ron(
        &dir.join(JOURNAL_FILE),
        &JournalSave {
            journal: save.script.journal.clone(),
        },
    )?;
    write_ron(
        &dir.join(GLOBALS_FILE),
        &GlobalsSave {
            globals: save.script.globals.clone(),
        },
    )?;
    write_ron(
        &dir.join(INSTANCES_FILE),
        &InstancesSave {
            instances: save.script.instances.clone(),
        },
    )?;
    write_ron(
        &dir.join(GUILDS_FILE),
        &GuildsSave {
            guild_attitudes: save.script.guild_attitudes.clone(),
        },
    )?;
    Ok(())
}

/// Reads a complete save directory into the in-memory aggregate. Version and
/// fingerprint checks are the caller's responsibility so it can report them
/// separately.
pub(crate) fn read_save(dir: &Path) -> Result<SaveGame, String> {
    let metadata = read_metadata(dir)?;
    let world: WorldSave = read_ron(&dir.join(WORLD_FILE))?;
    let npcs: NpcsSave = read_ron(&dir.join(NPCS_FILE))?;
    let items: ItemsSave = read_ron(&dir.join(ITEMS_FILE))?;
    let journal: JournalSave = read_ron(&dir.join(JOURNAL_FILE))?;
    let globals: GlobalsSave = read_ron(&dir.join(GLOBALS_FILE))?;
    let instances: InstancesSave = read_ron(&dir.join(INSTANCES_FILE))?;
    let guilds: GuildsSave = read_ron(&dir.join(GUILDS_FILE))?;

    Ok(SaveGame {
        metadata,
        script: ScriptSnapshot {
            globals: globals.globals,
            instances: instances.instances,
            npc_runtimes: npcs.npc_runtimes,
            item_runtimes: items.item_runtimes,
            journal: journal.journal,
            guild_attitudes: guilds.guild_attitudes,
            mob_routines: world.mob_routines,
        },
        objects: world.objects,
    })
}

/// Reads only `metadata.ron`, used for listing saves without loading the rest.
pub(crate) fn read_metadata(dir: &Path) -> Result<SaveMetadata, String> {
    read_ron(&dir.join(METADATA_FILE))
}

/// Fingerprint of the installed `GOTHIC.DAT`. Matches the value
/// `ScriptManager` stores, so saves can be validated before the script vm is
/// loaded (for example from the main menu).
pub(crate) fn current_script_fingerprint() -> Result<u64, String> {
    let path = gothic_dat_path();
    let bytes =
        fs::read(&path).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    Ok(fnv1a_64(&bytes))
}

/// Builds the path of a new save directory named after the current UTC time.
/// Returns the directory and the unix timestamp used in its name. Fails when a
/// directory with the same timestamp already exists; saves are never
/// overwritten.
pub(crate) fn new_save_dir() -> Result<(PathBuf, u64), String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock is before the unix epoch: {error}"))?;
    let saved_at_unix = now.as_secs();
    let dir = saves_dir().join(format!("{SAVE_PREFIX}{}", format_unix_utc(saved_at_unix)));
    if dir.exists() {
        return Err(format!("save directory {} already exists", dir.display()));
    }
    Ok((dir, saved_at_unix))
}

/// Every save directory in the save directory, newest first. Invalid metadata
/// is skipped with a warning. A missing directory is treated as "no saves".
pub(crate) fn list_saves() -> Vec<SaveEntry> {
    list_saves_in(&saves_dir())
}

fn list_saves_in(dir: &Path) -> Vec<SaveEntry> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(error) => {
            warn!("Failed to list save directory {}: {error}", dir.display());
            return Vec::new();
        }
    };
    let mut saves: Vec<SaveEntry> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| is_save_dir(path))
        .filter_map(|path| match read_metadata(&path) {
            Ok(metadata) => Some(SaveEntry { path, metadata }),
            Err(error) => {
                warn!("Skipping save {}: {error}", path.display());
                None
            }
        })
        .collect();
    // The timestamp in the name is fixed-width, so a reverse lexical sort is
    // also a newest-first chronological sort.
    saves.sort_by(|left, right| right.path.file_name().cmp(&left.path.file_name()));
    saves
}

/// The most recent valid save directory, if there is one.
pub(crate) fn newest_save() -> Option<PathBuf> {
    list_saves().into_iter().next().map(|entry| entry.path)
}

/// Human-readable label of a save directory: `save_2025-11-12_14-30-00` becomes
/// `2025-11-12 14-30-00`.
pub(crate) fn display_name(path: &Path) -> String {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return path.display().to_string();
    };
    let name = name.strip_prefix(SAVE_PREFIX).unwrap_or(name);
    name.replace('_', " ")
}

/// Whether `path` looks like a save directory this module wrote.
fn is_save_dir(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    path.is_dir() && name.starts_with(SAVE_PREFIX)
}

fn write_ron<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let mut config = ron::ser::PrettyConfig::default();
    config.depth_limit = 2;
    let contents = ron::ser::to_string_pretty(value, config)
        .map_err(|error| format!("failed to serialize {}: {error}", path.display()))?;
    fs::write(path, contents)
        .map_err(|error| format!("failed to write {}: {error}", path.display()))
}

fn read_ron<T: DeserializeOwned>(path: &Path) -> Result<T, String> {
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    ron::from_str(&contents).map_err(|error| format!("failed to parse {}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zengin::gameplay::game_time::GameTime;
    use crate::zengin::save::format::SAVE_FORMAT_VERSION;

    fn test_root(name: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("zengin_save_test_{}_{name}", std::process::id()));
        drop(fs::remove_dir_all(&root));
        root
    }

    fn metadata(saved_at_unix: u64) -> SaveMetadata {
        SaveMetadata {
            format_version: SAVE_FORMAT_VERSION,
            script_fingerprint: 42,
            world_name: "NEWWORLD/NEWWORLD".to_string(),
            saved_at_unix,
            game_time: GameTime::default(),
            play_time_seconds: 0.0,
            screenshot: None,
        }
    }

    fn save() -> SaveGame {
        SaveGame {
            metadata: metadata(1_762_957_800),
            script: ScriptSnapshot::default(),
            objects: Vec::new(),
        }
    }

    #[test]
    fn save_directory_round_trips() {
        let root = test_root("roundtrip");
        let dir = root.join("save_2025-11-12_14-30-00");
        write_save(&save(), &dir).expect("write save");
        let restored = read_save(&dir).expect("read save");

        assert_eq!(restored.metadata.world_name, "NEWWORLD/NEWWORLD");
        assert!(restored.objects.is_empty());
        assert!(restored.script.globals.is_empty());

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn list_skips_directories_without_metadata_and_sorts_newest_first() {
        let root = test_root("listing");
        fs::create_dir_all(&root).expect("create root");

        let older = root.join("save_2025-11-12_14-30-00");
        let newer = root.join("save_2025-11-13_10-00-00");
        let invalid = root.join("save_2025-11-14_10-00-00");
        for (dir, saved_at) in [(&older, 1_762_957_800), (&newer, 1_763_047_200)] {
            fs::create_dir_all(dir).expect("create save dir");
            write_ron(&dir.join(METADATA_FILE), &metadata(saved_at)).expect("write metadata");
        }
        fs::create_dir_all(&invalid).expect("create invalid dir");

        let saves = list_saves_in(&root);
        let names: Vec<_> = saves
            .iter()
            .filter_map(|entry| entry.path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            names,
            vec!["save_2025-11-13_10-00-00", "save_2025-11-12_14-30-00"]
        );

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn save_display_name_drops_prefix() {
        assert_eq!(
            display_name(Path::new("/saves/save_2025-11-12_14-30-00")),
            "2025-11-12 14-30-00"
        );
    }

    #[test]
    fn only_save_prefixed_directories_are_listed() {
        let root = test_root("dirs");
        let save_dir = root.join("save_2025-11-12_14-30-00");
        let tmp_dir = root.join(".tmp-save_2025-11-12_14-30-00");
        let plain_dir = root.join("quicksave");
        for dir in [&save_dir, &tmp_dir, &plain_dir] {
            fs::create_dir_all(dir).expect("create dir");
        }
        assert!(is_save_dir(&save_dir));
        assert!(!is_save_dir(&tmp_dir));
        assert!(!is_save_dir(&plain_dir));
        fs::remove_dir_all(root).expect("cleanup");
    }
}
