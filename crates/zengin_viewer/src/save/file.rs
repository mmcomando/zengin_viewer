//! Read, write and enumerate timestamped save files.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use bevy::log::warn;

use super::format::{SaveGame, fnv1a_64, saves_dir};

use crate::zengin::gothic2_dir;

/// Prefix of every save file this module manages. Other files in the save
/// directory (for example the legacy `quicksave.ron`) are ignored.
const SAVE_PREFIX: &str = "save_";

/// Writes a save file atomically: the RON goes to a temporary file next to the
/// target and is renamed over the final path only after a successful write.
pub(crate) fn write_save(save: &SaveGame, path: &Path) -> Result<(), String> {
    let Some(dir) = path.parent() else {
        return Err(format!(
            "save path({}) has no parent directory",
            path.display()
        ));
    };
    fs::create_dir_all(dir)
        .map_err(|error| format!("failed to create save directory {}: {error}", dir.display()))?;

    let mut config = ron::ser::PrettyConfig::default();
    config.depth_limit = 2;
    let contents = ron::ser::to_string_pretty(save, config)
        .map_err(|error| format!("failed to serialize save: {error}"))?;

    let tmp_path = path.with_extension("ron.tmp");
    fs::write(&tmp_path, contents)
        .map_err(|error| format!("failed to write {}: {error}", tmp_path.display()))?;
    fs::rename(&tmp_path, path).map_err(|error| {
        format!(
            "failed to move {} to {}: {error}",
            tmp_path.display(),
            path.display()
        )
    })?;
    Ok(())
}

/// Reads and parses a save file. Version and fingerprint checks are the
/// caller's responsibility so it can report them separately.
pub(crate) fn read_save(path: &Path) -> Result<SaveGame, String> {
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    ron::from_str(&contents).map_err(|error| format!("failed to parse {}: {error}", path.display()))
}

/// Fingerprint of the installed `GOTHIC.DAT`. Matches the value
/// `ScriptManager` stores, so saves can be validated before the script vm is
/// loaded (for example from the main menu).
pub(crate) fn current_script_fingerprint() -> Result<u64, String> {
    let path = PathBuf::from(format!(
        "{}/_work/Data/Scripts/_compiled/GOTHIC.DAT",
        gothic2_dir()
    ));
    let bytes =
        fs::read(&path).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    Ok(fnv1a_64(&bytes))
}

/// Builds the path of a new save named after the current UTC time. Fails when a
/// file with the same timestamp already exists; saves are never overwritten.
pub(crate) fn timestamped_save_path() -> Result<PathBuf, String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock is before the unix epoch: {error}"))?;
    let stamp = format_unix_utc(now.as_secs());
    let path = saves_dir().join(format!("{SAVE_PREFIX}{stamp}.ron"));
    if path.exists() {
        return Err(format!("save file {} already exists", path.display()));
    }
    Ok(path)
}

/// Every save file in the save directory, newest first. A missing directory is
/// treated as "no saves" instead of an error.
pub(crate) fn list_saves() -> Vec<PathBuf> {
    let dir = saves_dir();
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(error) => {
            warn!("Failed to list save directory {}: {error}", dir.display());
            return Vec::new();
        }
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && is_save_file(path))
        .collect();
    // The timestamp in the name is fixed-width, so a reverse lexical sort is
    // also a newest-first chronological sort.
    paths.sort_by(|left, right| right.file_name().cmp(&left.file_name()));
    paths
}

/// The most recent save, if there is one.
pub(crate) fn newest_save() -> Option<PathBuf> {
    list_saves().into_iter().next()
}

/// Whether `path` looks like a save file this module wrote.
fn is_save_file(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    path.extension().is_some_and(|extension| extension == "ron") && name.starts_with(SAVE_PREFIX)
}

/// Formats a unix timestamp as `YYYY-MM-DD_HH-MM-SS` in UTC.
fn format_unix_utc(seconds: u64) -> String {
    let days = (seconds / 86_400) as i64;
    let seconds_of_day = seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    format!("{year:04}-{month:02}-{day:02}_{hour:02}-{minute:02}-{second:02}")
}

/// Converts days since 1970-01-01 into a proleptic Gregorian calendar date.
/// Based on Howard Hinnant's `civil_from_days` algorithm.
fn civil_from_days(days_since_epoch: i64) -> (i64, u32, u32) {
    let shifted = days_since_epoch + 719_468;
    let era = if shifted >= 0 {
        shifted
    } else {
        shifted - 146_096
    } / 146_097;
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    };
    let year = if month <= 2 { year + 1 } else { year };
    (year, month as u32, day as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_epoch_formats_as_utc_date_and_time() {
        assert_eq!(format_unix_utc(0), "1970-01-01_00-00-00");
    }

    #[test]
    fn known_unix_timestamp_formats_as_utc() {
        // 2025-11-12 14:30:00 UTC
        assert_eq!(format_unix_utc(1_762_957_800), "2025-11-12_14-30-00");
    }

    #[test]
    fn leap_day_is_formatted_correctly() {
        // 2024-02-29 12:00:00 UTC
        assert_eq!(format_unix_utc(1_709_208_000), "2024-02-29_12-00-00");
    }

    #[test]
    fn saves_directory_points_into_the_workspace() {
        assert!(saves_dir().ends_with("misc/saves"));
    }

    #[test]
    fn only_save_prefixed_ron_files_are_listed() {
        assert!(is_save_file(Path::new(
            "/saves/save_2025-11-12_14-30-00.ron"
        )));
        assert!(!is_save_file(Path::new("/saves/quicksave.ron")));
        assert!(!is_save_file(Path::new("/saves/save_2025-11-12.ron.tmp")));
        assert!(!is_save_file(Path::new("/saves/notes.txt")));
    }
}
