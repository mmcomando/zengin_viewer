//! Body state queries and overrides driven by mob interaction.

use super::super::state::{MobSchemeLists, body_state_for_scheme};
use super::super::symbol_id::InstanceId;
use super::ScriptManager;

impl ScriptManager {
    /// Resolves the body state of a mob from the `MOB_*` scheme lists of the
    /// loaded scripts, falling back to the engine defaults when a script does
    /// not declare them.
    pub fn scheme_body_state(&mut self, scheme: &str) -> i32 {
        let mut lists = MobSchemeLists::default();
        for (symbol, list) in [
            ("mob_sit", &mut lists.sit),
            ("mob_lie", &mut lists.lie),
            ("mob_climb", &mut lists.climb),
            ("mob_notinterruptable", &mut lists.not_interruptable),
        ] {
            if let Some(value) = self.string_variable(symbol) {
                *list = value;
            }
        }
        body_state_for_scheme(scheme, &lists)
    }

    /// Sets or clears the body state override of an npc.
    pub fn set_npc_body_state(&mut self, npc: InstanceId, body_state: Option<i32>) {
        self.npc_state_mut(npc).live.body_state = body_state;
    }
}
