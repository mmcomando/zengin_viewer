//! Script-side mirror of the way-net freepoints and their occupancy.

use super::ScriptManager;
use crate::script::state::ScriptFreePoint;
use crate::script::symbol_id::InstanceId;

impl ScriptManager {
    /// Replaces the freepoint table.
    pub fn set_freepoints(&mut self, freepoints: Vec<ScriptFreePoint>) {
        self.store.data_mut().freepoints = freepoints;
    }

    /// Frees the freepoint held by the npc.
    pub fn release_freepoint(&mut self, npc: InstanceId) {
        self.store.data_mut().release_freepoint(npc);
    }
}
