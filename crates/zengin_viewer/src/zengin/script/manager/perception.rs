//! Engine side of the npc perception loop: finds what an npc senses and calls
//! the script callback the state registered with `Npc_PercEnable`.

use bevy::log::warn;

use super::ScriptManager;
use crate::zengin::script::state::PERC_ASSESSENEMY;
use crate::zengin::script::symbol_id::{FunctionId, InstanceId};

/// Scan cadence before a state calls `Npc_SetPercTime`, matching the engine
/// constructor default of five seconds.
pub const DEFAULT_PERCEPTION_INTERVAL_SECS: f32 = 5.0;

impl ScriptManager {
    /// Seconds between perception scans of the npc (`Npc_SetPercTime`), the
    /// engine default when the script did not set one.
    pub fn perception_interval(&self, npc: InstanceId) -> f32 {
        self.store
            .data()
            .npc_state(npc)
            .and_then(|npc| npc.perception.interval)
            .unwrap_or(DEFAULT_PERCEPTION_INTERVAL_SECS)
    }

    /// Runs one perception pass for `npc`: when the npc senses an enemy and the
    /// script registered a `PERC_ASSESSENEMY` callback, the callback runs with
    /// `self` bound to the npc and `other` to the enemy. `victim` is cleared,
    /// the enemy perception has no second actor.
    pub fn run_perception(&mut self, npc: InstanceId) {
        if Some(npc) == self.store.data().player_instance {
            return;
        }
        let Some(callback) = self.perception_callback(npc, PERC_ASSESSENEMY) else {
            bevy::log::trace!(
                "perception skip npc({}) no PERC_ASSESSENEMY callback, start_aistate({:?})",
                npc.value(),
                self.store
                    .data()
                    .typed
                    .npc(npc.value() as u32)
                    .and_then(|npc| npc.start_aistate)
                    .map(|state| state.value()),
            );
            return;
        };
        let Some(enemy) = self.store.data().nearest_enemy(npc) else {
            let data = self.store.data();
            let position = data.npc_state(npc).and_then(|npc| npc.live.position);
            let player_position = data
                .player_instance
                .and_then(|player| data.npc_state(player))
                .and_then(|player| player.live.position);
            bevy::log::trace!(
                "perception npc({}) sees no enemy, position({position:?}) player_position({player_position:?})",
                npc.value(),
            );
            return;
        };
        let attitude = self.store.data().npc_attitude(npc, enemy, true);
        let guilds = (
            self.store
                .data()
                .typed
                .npc(npc.value() as u32)
                .map(|npc| npc.guild),
            self.store
                .data()
                .typed
                .npc(enemy.value() as u32)
                .map(|npc| npc.guild),
        );
        bevy::log::debug!(
            "perception npc({}) sees enemy({}) attitude({attitude}) guilds({guilds:?})",
            npc.value(),
            enemy.value(),
        );
        self.call_perception(npc, enemy, callback);
    }

    fn perception_callback(&self, npc: InstanceId, perception: i32) -> Option<FunctionId> {
        let runtime = self.store.data().npc_state(npc)?;
        runtime.perception.callbacks.get(&perception).copied()
    }

    /// Binds the perception scope (`self`, `other`, `victim`) and runs the
    /// callback, the way the engine dispatches a passive perception.
    fn call_perception(&mut self, npc: InstanceId, other: InstanceId, function: FunctionId) {
        self.set_self(npc);
        self.set_instance(npc);
        self.set_other(other);
        self.set_victim(None);
        if let Err(error) = self.try_call_function(function) {
            warn!(
                "Perception npc({}) other({}) function({}) failed: {error:?}",
                npc.value(),
                other.value(),
                function.value()
            );
        }
    }
}
