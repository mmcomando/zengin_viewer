use super::*;

impl mmcomando::zengine_scripts::wld_scripts::Host for ImportCallState {
    fn wld_insertnpc(&mut self, npc_index: i32, waypoint_string_id: i32) {
        let Some(npc_index) = self.symbol_table.instance(npc_index) else {
            return;
        };
        let Some(waypoint_string_id) = self.symbol_table.string(waypoint_string_id) else {
            return;
        };
        // println!(
        //     "NEW wld_insertnpc npc_index({npc_index}), waypoint_string_id({waypoint_string_id})"
        // );

        self.pending.spawn_npcs.push(SpawnNpc {
            instance_id: npc_index,
            transform: None,
            exact_transform: false,
            overlay: None,
            location_name: Some(waypoint_string_id),
        });
    }
    fn wld_insertitem(&mut self, item_index: i32, waypoint_string_id: i32) {
        // println!("CCC");
        let Some(item_index) = self.symbol_table.instance(item_index) else {
            return;
        };
        let Some(waypoint_string_id) = self.symbol_table.string(waypoint_string_id) else {
            return;
        };
        // println!(
        //     "NEW wld_insertitem item_index({item_index}), item_name({item_name}), waypoint_string_id({waypoint_string_id}), waypoint_name({waypoint_name})"
        // );

        self.pending.spawn_items.push(SpawnItem {
            instance_id: item_index,
            transform: None,
            location_name: Some(waypoint_string_id),
        });
    }
    fake_impl!(wld_detectitem(_0, _1) -> i32);
    fake_impl!(wld_detectplayer(_0) -> i32);
    fake_impl!(wld_detectnpc(_0, _1, _2, _3) -> i32);
    fake_impl!(wld_detectnpcex(_0, _1, _2, _3, _4) -> i32);
    fake_impl!(wld_detectnpcexatt(_0, _1, _2, _3, _4, _5) -> i32);
    fn wld_setguildattitude(&mut self, guild_a: i32, guild_b: i32, attitude: i32) {
        self.guild_attitudes.insert((guild_a, guild_b), attitude);
    }
    fake_impl!(wld_getguildattitude(_0, _1) -> i32);
    /// Minimal mob availability: all mobs count as available so routine states
    /// reach their use-mob branch (real mob objects are not simulated)
    fn wld_ismobavailable(&mut self, _npc: i32, _mob_name: i32) -> i32 {
        warn_unimplemented!("wld_ismobavailable");
        1
    }
    fake_impl!(wld_isfpavailable(_0, _1) -> i32);
    fake_impl!(wld_isnextfpavailable(_0, _1) -> i32);
    fake_impl!(wld_getmobstate(_0, _1) -> i32);
    fake_impl!(wld_istime(_0, _1, _2, _3) -> i32);
    /// Day counted from 0 on the first day.
    fn wld_getday(&mut self) -> i32 {
        self.game_day as i32
    }
    fake_impl!(wld_israining() -> i32);
    fake_impl!(wld_settime(_0, _1));
    fake_impl!(wld_spawnnpcrange(_0, _1, _2, _3));
    fake_impl!(wld_playeffect(_0, _1, _2, _3, _4, _5, _6));
    fake_impl!(wld_stopeffect(_0));
    fake_impl!(wld_removenpc(_0));
    fake_impl!(wld_insertnpcandrespawn(_0, _1, _2));
    fake_impl!(wld_insertobject(_0, _1));
    fake_impl!(wld_removeitem(_0) -> i32);
    fake_impl!(wld_exchangeguildattitudes(_0));
    fake_impl!(wld_setobjectroutine(_0, _1, _2, _3));
    /// Records the mob state assignment for later consumers, mobs are not
    /// simulated yet so nothing acts on the stored entries
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
