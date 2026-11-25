//! Player-side inventory and talent checks for chest locks.

use bevy::log::warn;

use super::ScriptManager;
use crate::zengin::script::symbol_id::InstanceId;

/// Talent slot `NPC_TALENT_PICKLOCK` of the Gothic 2 script constants.
pub(crate) const TALENT_PICKLOCK: i32 = 5;

/// Script instance name of the lockpick item.
pub(crate) const LOCKPICK_ITEM: &str = "itke_lockpick";

impl ScriptManager {
    pub fn talent_skill(&self, npc: InstanceId, talent: i32) -> i32 {
        self.npc_state(npc)
            .map_or(0, |npc| npc.talents.skill(talent))
    }

    pub fn item_count(&self, npc: InstanceId, item: InstanceId) -> i32 {
        self.npc_state(npc).map_or(0, |npc| {
            npc.inventory
                .items()
                .into_iter()
                .find(|(held, _)| *held == item)
                .map_or(0, |(_, count)| count)
        })
    }

    pub fn has_lockpick(&mut self, npc: InstanceId) -> bool {
        let Some(item) = self.symbol_instance(LOCKPICK_ITEM) else {
            return false;
        };
        self.item_count(npc, item) > 0
    }

    /// Runs `g_picklock` with `player` bound as `self`. The script plays the
    /// result sound and prints the matching message; `broken` selects the
    /// broken-pick variants.
    pub fn call_picklock(&mut self, player: InstanceId, success: bool, broken: bool) {
        let Some(function) = self.try_get_function_id("g_picklock") else {
            warn!("picklock function(g_picklock) is not declared");
            return;
        };
        self.set_self(player);
        self.set_instance(player);
        if let Err(err) =
            self.try_call_function_args2(function, i32::from(success), i32::from(broken))
        {
            warn!("g_picklock player({player}) failed: {err:?}");
        }
    }

    /// Runs a `player_mob_*` refusal with `player` bound as `self`.
    pub fn call_player_refusal(&mut self, player: InstanceId, function: &str) {
        let Some(function_id) = self.try_get_function_id(function) else {
            warn!("player refusal function({function}) is not declared");
            return;
        };
        self.set_self(player);
        self.set_instance(player);
        if let Err(err) = self.try_call_function(function_id) {
            warn!("player refusal function({function}) npc({player}) failed: {err:?}");
        }
    }
}
