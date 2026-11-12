//! Quick save/load: on-disk format, file IO and the save/load systems.

mod file;
pub(crate) mod format;
mod systems;

pub(crate) use file::{
    current_script_fingerprint, newest_save, read_save, timestamped_save_path, write_save,
};
pub(crate) use systems::{PendingLoad, QuickLoad, QuickSave, QuickSavePlugin};
