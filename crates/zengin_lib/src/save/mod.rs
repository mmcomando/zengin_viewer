//! Quick save/load: on-disk format, file IO and the save/load systems.

#![allow(clippy::multiple_inherent_impl)]

pub(crate) mod file;
pub(crate) mod format;
pub(crate) mod object;
pub(crate) mod systems;

pub use file::{
    SaveEntry, current_script_fingerprint, display_name, list_saves, new_save_dir, newest_save,
    read_save, write_save,
};
pub use systems::{LoadSaveRequest, QuickLoad, QuickSave, QuickSavePlugin};
