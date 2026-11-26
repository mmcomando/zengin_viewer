//! Input profile loading from the `input` assets subdirectory.

use std::path::Path;

use leafwing_input_manager::prelude::{Actionlike, InputMap};
use serde::Serialize;
use serde::de::DeserializeOwned;

/// Loads the input profile RON file `<assets_dir>/input/<file_name>`.
pub fn load_input_profile<A>(assets_dir: &Path, file_name: &str) -> Result<InputMap<A>, String>
where
    A: Actionlike + Serialize + DeserializeOwned,
{
    let path = assets_dir.join("input").join(file_name);
    let contents = std::fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    ron::de::from_str(&contents)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))
}
