//! Engine side of npc death: runs the `ZS_DEAD` script state the way the
//! engine's `onNoHealth` does, so xp, quest counters and kill flags update.

use bevy::log::warn;

use super::super::symbol_id::InstanceId;

impl super::ScriptManager {
    /// Runs the `ZS_DEAD` state for a dying npc. The previous state's end hook
    /// runs first (with the killer bound as `other`), then the death state is
    /// installed as the current state and its init and one loop step run.
    ///
    /// The npc position is still valid during the calls so scripts can measure
    /// distances; the caller clears it afterwards. `None` clears `other`, like
    /// the engine's `onNoHealth` with no `lastHit`.
    pub fn call_death_state(&mut self, npc: InstanceId, killer: Option<InstanceId>) {
        match killer {
            Some(killer) => self.set_other(killer),
            None => self.clear_other(),
        }
        self.set_victim(None);

        let death = self.try_get_function_id("zs_dead");
        let previous = self
            .npc_state(npc)
            .and_then(|state| state.routine.current_state)
            .filter(|previous| Some(*previous) != death);
        if let Some(previous) = previous {
            let functions = self.resolve_routine_functions(previous);
            self.call_routine_end(npc, functions);
        }

        // Liveness checks (`Npc_IsDead`, `c_npcisdown`) read this.
        self.npc_state_mut(npc).live.dead = true;

        let Some(death) = death else {
            warn!(
                "No zs_dead script function, npc({}) dies without death scripts",
                npc.value()
            );
            return;
        };
        let functions = self.resolve_routine_functions(death);
        self.call_routine_start(npc, functions);
        // The engine keeps looping the dead state while the corpse exists. The
        // viewer despawns the npc, one step is enough for its one-time work.
        self.call_routine_loop(npc, functions);
    }
}
