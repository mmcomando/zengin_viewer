//! Engine side of the npc perception loop: finds what an npc senses and calls
//! the script callback the state registered with `Npc_PercEnable`.

use bevy::log::warn;

use super::ScriptManager;
use crate::zengin::common::to_world_units;
use crate::zengin::script::state::{
    NpcCheckInfo, PERC_ASSESSDAMAGE, PERC_ASSESSENEMY, PERC_ASSESSPLAYER,
};
use crate::zengin::script::symbol_id::{FunctionId, InstanceId};

/// Scan cadence before a state calls `Npc_SetPercTime`, matching the engine
/// constructor default of five seconds.
pub const DEFAULT_PERCEPTION_INTERVAL_SECS: f32 = 5.0;

/// Script `PERC_DIST_DIALOG`, the range within which `b_assessplayer` evaluates
/// `Npc_CheckInfo` and may start a talk.
const PERC_DIST_DIALOG: f32 = 500.0;

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

    /// Runs one perception pass for `npc`: senses are dispatched to the
    /// callbacks the script registered with `Npc_PercEnable`. `auto_talk` gates
    /// the `PERC_ASSESSPLAYER` dispatch, which is the one that starts npc
    /// initiated dialogs.
    pub fn run_perception(&mut self, npc: InstanceId, auto_talk: bool) {
        if Some(npc) == self.store.data().player_instance {
            return;
        }
        if self
            .store
            .data()
            .npc_state(npc)
            .is_some_and(|npc| npc.live.dead)
        {
            return;
        }
        self.run_enemy_perception(npc);
        if auto_talk {
            self.run_player_perception(npc);
        }
    }

    /// Dispatches `PERC_ASSESSDAMAGE` to the npc that took damage. This is an
    /// event-driven perception with no range check.
    pub fn run_damage_perception(&mut self, victim: InstanceId, attacker: InstanceId) {
        if Some(victim) == self.store.data().player_instance {
            return;
        }
        if self
            .store
            .data()
            .npc_state(victim)
            .is_none_or(|npc| npc.live.dead)
        {
            return;
        }
        let Some(callback) = self.perception_callback(victim, PERC_ASSESSDAMAGE) else {
            return;
        };
        bevy::log::debug!(
            "damage perception npc({}) attacker({})",
            victim.value(),
            attacker.value(),
        );
        self.call_perception(victim, attacker, None, callback);
    }

    /// Dispatches a passive perception broadcast by `source`
    /// (`Npc_SendPassivePerc`) to `receiver` when it has the callback
    /// registered for `perception` and the source is within its perception
    /// range. The receiver must be alive and not the player; the caller gates
    /// on the `AiNormal` policy and runs this for every nearby npc.
    pub fn run_passive_perception(
        &mut self,
        receiver: InstanceId,
        source: InstanceId,
        other: InstanceId,
        victim: Option<InstanceId>,
        perception: i32,
    ) {
        if Some(receiver) == self.store.data().player_instance {
            return;
        }
        let Some(callback) = self.perception_callback(receiver, perception) else {
            return;
        };
        let (receiver_position, source_position, senses_range) = {
            let data = self.store.data();
            let Some(state) = data.npc_state(receiver) else {
                return;
            };
            if state.live.dead {
                return;
            }
            let Some(receiver_position) = state.live.position else {
                return;
            };
            let Some(source_position) = data.npc_state(source).and_then(|npc| npc.live.position)
            else {
                return;
            };
            let senses_range = data
                .typed
                .npc(receiver.value() as u32)
                .map_or(0, |npc| npc.senses_range);
            (receiver_position, source_position, senses_range)
        };
        let range = self.store.data().perc_range(perception, senses_range);
        let range = to_world_units(range as f32);
        let distance_squared = (source_position - receiver_position).length_squared();
        if range <= 0.0 || distance_squared > range * range {
            return;
        }
        bevy::log::debug!(
            "passive perception receiver({}) perception({perception}) source({}) distance({:.2})",
            receiver.value(),
            source.value(),
            distance_squared.sqrt(),
        );
        self.call_perception(receiver, other, victim, callback);
    }

    /// Dispatches `PERC_ASSESSPLAYER` when the player is within the npc senses
    /// range. Conditions of the dialog infos are evaluated only inside the
    /// dialog distance, matching the short-circuit in `b_assessplayer`.
    fn run_player_perception(&mut self, npc: InstanceId) {
        let Some(callback) = self.perception_callback(npc, PERC_ASSESSPLAYER) else {
            return;
        };
        let (player, position, player_position, senses_range) = {
            let data = self.store.data();
            let Some(player) = data.player_instance else {
                return;
            };
            let Some(position) = data.npc_state(npc).and_then(|npc| npc.live.position) else {
                return;
            };
            let Some(player_position) = data.npc_state(player).and_then(|npc| npc.live.position)
            else {
                return;
            };
            let senses_range = data
                .typed
                .npc(npc.value() as u32)
                .map_or(0, |npc| npc.senses_range);
            (player, position, player_position, senses_range)
        };
        let range = self
            .store
            .data()
            .perc_range(PERC_ASSESSPLAYER, senses_range);
        if range <= 0 {
            return;
        }
        let range = to_world_units(range as f32);
        let distance_squared = (player_position - position).length_squared();
        if distance_squared > range * range {
            return;
        }
        let dialog_range = to_world_units(PERC_DIST_DIALOG);
        if distance_squared <= dialog_range * dialog_range
            && self.refresh_npc_check_info(npc, player).is_none()
        {
            // `Npc_CheckInfo` answers from the cache; keep it populated even for
            // npcs without dialog roots so the script never hits the stale warn.
            self.store
                .data_mut()
                .check_info
                .insert(npc, NpcCheckInfo::default());
        }
        bevy::log::debug!(
            "perception npc({}) assesses player({}) distance({:.2})",
            npc.value(),
            player.value(),
            distance_squared.sqrt(),
        );
        self.call_perception(npc, player, None, callback);
    }

    /// Dispatches `PERC_ASSESSENEMY` when the npc senses an enemy.
    fn run_enemy_perception(&mut self, npc: InstanceId) {
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
        self.call_perception(npc, enemy, None, callback);
    }

    fn perception_callback(&self, npc: InstanceId, perception: i32) -> Option<FunctionId> {
        let runtime = self.store.data().npc_state(npc)?;
        runtime.perception.callbacks.get(&perception).copied()
    }

    /// Binds the perception scope (`self`, `other`, `victim`) and runs the
    /// callback, the way the engine dispatches a perception.
    fn call_perception(
        &mut self,
        npc: InstanceId,
        other: InstanceId,
        victim: Option<InstanceId>,
        function: FunctionId,
    ) {
        self.set_self(npc);
        self.set_instance(npc);
        self.set_other(other);
        self.set_victim(victim);
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
