//! Read and write the single quicksave slot.

use std::fs;
use std::path::PathBuf;

use super::format::{SaveGame, save_path};

/// Writes the quicksave atomically: the RON goes to a temporary file next to
/// the target and is renamed over the final path only after a successful write.
pub(crate) fn write_save(save: &SaveGame) -> Result<PathBuf, String> {
    let path = save_path();
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
    fs::rename(&tmp_path, &path).map_err(|error| {
        format!(
            "failed to move {} to {}: {error}",
            tmp_path.display(),
            path.display()
        )
    })?;
    Ok(path)
}

/// Reads and parses the quicksave. Version and fingerprint checks are the
/// caller's responsibility so it can report them separately.
pub(crate) fn read_save() -> Result<SaveGame, String> {
    let path = save_path();
    let contents = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    ron::from_str(&contents).map_err(|error| format!("failed to parse {}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_path_points_into_the_workspace_save_directory() {
        let path = save_path();
        assert!(path.ends_with("misc/saves/quicksave.ron"));
    }
}
