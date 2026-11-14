//! Script-side mirror of the interactive mobs.

use super::ScriptManager;
use crate::zengin::script::state::ScriptMob;

impl ScriptManager {
    /// Replaces the mob table. Keys must be lowercased schemes.
    pub fn set_mobs(&mut self, mobs: Vec<(String, ScriptMob)>) {
        let mut by_scheme = std::collections::HashMap::new();
        for (scheme, mob) in mobs {
            by_scheme.entry(scheme).or_insert_with(Vec::new).push(mob);
        }
        self.store.data_mut().mobs = by_scheme;
    }
}
