//! Quick save/load: on-disk format, file IO and the save/load systems.

mod file;
pub(crate) mod format;

pub(crate) use file::{read_save, write_save};
