use super::*;

impl mmcomando::zengine_scripts::ta_scripts::Host for ImportCallState {
    fn ta_min(
        &mut self,
        npc_index: i32,
        start_h: i32,
        start_m: i32,
        stop_h: i32,
        stop_m: i32,
        func_index: i32,
        way_point: i32,
    ) {
        // println!(
        //     "NEW ta_min npc_index({npc_index}), start_h({start_h}), stop_h({stop_h}), way_point({way_point})"
        // );
        let Some(npc_index) = self.symbol_table.instance(npc_index) else {
            return;
        };
        let Some(way_point) = self.symbol_table.string(way_point) else {
            return;
        };
        let Some(way_point) = self.get_canon_string(way_point) else {
            return;
        };

        let routine_entry = RoutineEntry {
            start_h: start_h.max(0) as u32,
            start_m: start_m.max(0) as u32,
            stop_h: stop_h.max(0) as u32,
            stop_m: stop_m.max(0) as u32,
            way_point,
            // Id 0 means the state has no function
            state_fn: self.symbol_table.function(func_index),
        };

        self.npc_state_mut(npc_index)
            .routine
            .entries
            .push(routine_entry);
    }
    fake_impl!(ta(_0, _1, _2, _3, _4));
    fake_impl!(ta_cs(_0, _1, _2));
    fake_impl!(ta_beginoverlay(_0));
    fake_impl!(ta_endoverlay(_0));
    fake_impl!(ta_removeoverlay(_0));
}
