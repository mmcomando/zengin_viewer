#![allow(clippy::multiple_inherent_impl)]

use super::*;
use crate::save::format::SavedObjectBuilder;
use crate::zengin::SpawnFromSavedObject;
use crate::zengin::common::to_world_units;
use bevy::prelude::Transform;
use dev_util::warn_unimplemented;

impl mmcomando::zengine_scripts::wld_scripts::Host for ImportCallState {
    fn wld_insertnpc(&mut self, npc_index: i32, waypoint_string_id: i32) {
        let Some(source) = self.symbol_table.instance(npc_index) else {
            return;
        };
        let Some(waypoint_string_id) = self.symbol_table.string(waypoint_string_id) else {
            return;
        };
        let Some(place) = self.get_canon_string(waypoint_string_id) else {
            return;
        };
        // The same instance can be inserted several times (`ywolf` and friends).
        // Every call is a separate script object, only the first one keeps the
        // symbol id.
        let object = self.allocate_npc_object(source);
        // The engine constructor seeds `C_Npc.wp` with the spawn waypoint; the
        // routine state start updates it to the active routine waypoint later.
        self.typed
            .set_npc_waypoint(object.value() as u32, Some(waypoint_string_id));
        self.messages.spawn_objects.send(SpawnFromSavedObject {
            object: SavedObjectBuilder::new(Transform::IDENTITY)
                .spawn_at_place(place.as_str().to_string())
                .npc(object)
                .build(),
        });
    }
    fn wld_insertitem(&mut self, item_index: i32, waypoint_string_id: i32) {
        let Some(item_index) = self.symbol_table.instance(item_index) else {
            return;
        };
        let Some(waypoint_string_id) = self.symbol_table.string(waypoint_string_id) else {
            return;
        };
        let Some(place) = self.get_canon_string(waypoint_string_id) else {
            return;
        };
        self.messages.spawn_objects.send(SpawnFromSavedObject {
            object: SavedObjectBuilder::new(Transform::IDENTITY)
                .spawn_at_place(place.as_str().to_string())
                .item(item_index)
                .build(),
        });
    }
    fake_impl!(wld_detectitem(_0, _1) -> i32);
    fake_impl!(wld_detectplayer(_0) -> i32);
    /// Nearest matching npc within the source npc's `senses_range` becomes the
    /// `other` global.
    fn wld_detectnpc(&mut self, npc: i32, instance: i32, state: i32, guild: i32) -> i32 {
        self.detect_npc(npc, instance, state, guild, false)
    }
    /// Like `Wld_DetectNpc`, excluding the player when `detect_player` is zero.
    fn wld_detectnpcex(
        &mut self,
        npc: i32,
        instance: i32,
        state: i32,
        guild: i32,
        detect_player: i32,
    ) -> i32 {
        self.detect_npc(npc, instance, state, guild, detect_player == 0)
    }
    fake_impl!(wld_detectnpcexatt(_0, _1, _2, _3, _4, _5) -> i32);
    fn wld_setguildattitude(&mut self, guild_a: i32, attitude: i32, guild_b: i32) {
        self.guild_attitudes.insert((guild_a, guild_b), attitude);
    }
    /// Guild matrix lookup, `ATT_HOSTILE` when the pair was never set.
    fn wld_getguildattitude(&mut self, guild_a: i32, guild_b: i32) -> i32 {
        self.guild_attitudes
            .get(&(guild_a, guild_b))
            .copied()
            .unwrap_or(ATT_HOSTILE)
    }
    /// `Wld_IsMobAvailable`: the npc has a free mob with the scheme nearby.
    fn wld_ismobavailable(&mut self, npc: i32, mob_name: i32) -> i32 {
        let Some(npc) = self.symbol_table.instance(npc) else {
            return 0;
        };
        let Some(scheme) = self.mob_scheme(mob_name) else {
            return 0;
        };
        i32::from(self.available_mob(npc, scheme.as_str()).is_some())
    }
    /// `Wld_IsFPAvailable`: the npc has a free freepoint with the name nearby.
    fn wld_isfpavailable(&mut self, npc: i32, waypoint: i32) -> i32 {
        self.freepoint_available(npc, waypoint, false)
    }
    /// `Wld_IsNextFPAvailable`: like above, skipping the npc's current point.
    fn wld_isnextfpavailable(&mut self, npc: i32, waypoint: i32) -> i32 {
        self.freepoint_available(npc, waypoint, true)
    }
    /// State of the nearest free mob with the scheme, `-1` when there is none.
    fn wld_getmobstate(&mut self, npc: i32, mob_name: i32) -> i32 {
        let Some(npc) = self.symbol_table.instance(npc) else {
            return -1;
        };
        let Some(scheme) = self.mob_scheme(mob_name) else {
            return -1;
        };
        self.available_mob(npc, scheme.as_str())
            .map_or(-1, |mob| mob.state.max(0))
    }
    /// In-game time window check, the range may wrap over midnight.
    fn wld_istime(
        &mut self,
        hour_start: i32,
        minute_start: i32,
        hour_end: i32,
        minute_end: i32,
    ) -> i32 {
        let begin = hour_start * 60 + minute_start;
        let end = hour_end * 60 + minute_end;
        let now = (self.game_minutes_of_day % (24 * 60)) as i32;
        let inside = if begin <= end {
            begin <= now && now < end
        } else {
            now < end || begin <= now
        };
        i32::from(inside)
    }
    /// Day counted from 0 on the first day.
    fn wld_getday(&mut self) -> i32 {
        self.game_day as i32
    }
    fake_impl!(wld_israining() -> i32);
    /// Moves the world clock to the next occurrence of `hour:minute`.
    fn wld_settime(&mut self, hour: i32, minute: i32) {
        self.messages.game_time.send(SetGameTime { hour, minute });
    }
    fake_impl!(wld_spawnnpcrange(_0, _1, _2, _3));
    fake_impl!(wld_playeffect(_0, _1, _2, _3, _4, _5, _6));
    fake_impl!(wld_stopeffect(_0));
    fake_impl!(wld_removenpc(_0));
    fake_impl!(wld_insertnpcandrespawn(_0, _1, _2));
    fake_impl!(wld_insertobject(_0, _1));
    fake_impl!(wld_removeitem(_0) -> i32);
    /// Rebuilds the guild attitude matrix from a square script array symbol
    /// (`Wld_ExchangeGuildAttitudes("GIL_ATTITUDES")`), the engine calls it once
    /// at session start.
    fn wld_exchangeguildattitudes(&mut self, name: i32) {
        let Some(name) = self.canon_string_arg(name) else {
            warn!("Wld_ExchangeGuildAttitudes with invalid name({name})");
            return;
        };
        let Some(symbol) = self.symbol_table.symbol_id_by_name(name.as_str()) else {
            warn!("Wld_ExchangeGuildAttitudes symbol({name}) not found");
            return;
        };
        self.exchange_guild_attitudes(symbol as u32);
    }
    fake_impl!(wld_setobjectroutine(_0, _1, _2, _3));
    /// Records the mob state assignment for the world side, which applies it at
    /// the scheduled time
    fn wld_setmobroutine(&mut self, hour: i32, minute: i32, mob_name: i32, state: i32) {
        let (hour, minute, state) = (hour, minute, state);
        let Some(mob_name_id) = self.symbol_table.string(mob_name) else {
            warn_unimplemented!("wld_setmobroutine with non-string mob name({mob_name})");
            return;
        };
        let Some(mob_name) = self.get_string_non_0(mob_name_id.value()) else {
            warn_unimplemented!("wld_setmobroutine with empty mob name");
            return;
        };
        bevy::log::debug!("Wld_SetMobRoutine {hour}:{minute:02} {mob_name} -> state({state})");
        self.loaded_state.mob_routines.push(MobRoutineEntry {
            hour,
            minute,
            mob_name,
            state,
        });
    }
    fake_impl!(wld_sendtrigger(_0));
    fake_impl!(wld_senduntrigger(_0));
    fake_impl!(wld_assignroomtoguild(_0, _1));
    fake_impl!(wld_assignroomtonpc(_0, _1));
    fake_impl!(wld_getplayerportalowner() -> i32);
    fake_impl!(wld_getplayerportalguild() -> i32);
    fake_impl!(wld_getformerplayerportalowner() -> i32);
    fake_impl!(wld_getformerplayerportalguild() -> i32);
}

impl ImportCallState {
    /// `-1` in `instance`, `state` and `guild` means "any", as in
    /// `Wld_DetectNpc`.
    fn detect_npc(
        &mut self,
        npc: i32,
        instance: i32,
        state: i32,
        guild: i32,
        exclude_player: bool,
    ) -> i32 {
        let Some(npc) = self.symbol_table.instance(npc) else {
            return 0;
        };
        let Some(npc_data) = self.typed.npc(npc.value() as u32) else {
            warn!("Wld_DetectNpc: npc({}) has no C_Npc data", npc.value());
            return 0;
        };
        let radius = to_world_units(npc_data.senses_range as f32);
        let instance = if instance == -1 {
            None
        } else {
            let Some(instance) = self.symbol_table.instance(instance) else {
                return 0;
            };
            Some(instance)
        };
        let state = if state == -1 {
            None
        } else {
            let Some(state) = self.symbol_table.function(state) else {
                return 0;
            };
            Some(state)
        };
        let guild = (guild != -1).then_some(guild);
        let Some(other) = self.nearest_npc(npc, radius, instance, state, guild, exclude_player)
        else {
            return 0;
        };
        self.set_other(other);
        1
    }
}
