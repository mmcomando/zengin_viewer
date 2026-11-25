//! Quick save/load: on-disk format, file IO and the save/load systems.

mod file;
pub(crate) mod format;
mod object;
mod systems;

pub(crate) use file::{
    SaveEntry, current_script_fingerprint, display_name, list_saves, new_save_dir, newest_save,
    read_save, write_save,
};
pub(crate) use systems::{LoadSaveRequest, PendingLoad, QuickLoad, QuickSave, QuickSavePlugin};
