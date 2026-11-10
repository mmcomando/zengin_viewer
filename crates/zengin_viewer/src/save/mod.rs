//! Quick save/load: on-disk format, file IO and the save/load systems.

mod file;
pub(crate) mod format;
mod systems;

pub(crate) use file::{read_save, write_save};
pub(crate) use systems::{QuickLoad, QuickSave, QuickSavePlugin};
