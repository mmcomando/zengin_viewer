use super::*;

impl mmcomando::zengine_scripts::hlp_scripts::Host for ImportCallState {
    /// Returns whether the two string symbols hold equal text.
    ///
    /// The comparison is case insensitive: the viewer keeps canonical
    /// lowercase waypoint keys while script literals keep their original case,
    /// so `Hlp_StrCmp(Npc_GetNearestWP(self), self.wp)` would otherwise never
    /// match (the retail `Gothic::hlp_strcmp` is an exact comparison).
    fn hlp_strcmp(&mut self, aa: i32, bb: i32) -> i32 {
        let Some(aa) = self.symbol_table.string(aa) else {
            return 0;
        };
        let Some(bb) = self.symbol_table.string(bb) else {
            return 0;
        };
        let aa = self.get_show_string(aa);
        let bb = self.get_show_string(bb);
        match (aa, bb) {
            // Both empty compares equal, like the previous `Option<String>`
            // equality did
            (None, None) => 1,
            (Some(aa), Some(bb)) => i32::from(aa.string().eq_ignore_ascii_case(bb.string())),
            _ => 0,
        }
    }
    /// `1` when both arguments are the same item instance symbol. The viewer
    /// stores inventory items as their class instance id, so this is an id
    /// equality check.
    fn hlp_isitem(&mut self, item: i32, instance_symbol: i32) -> i32 {
        let Some(item) = self.symbol_table.instance(item) else {
            return 0;
        };
        let Some(instance) = self.symbol_table.instance(instance_symbol) else {
            return 0;
        };
        i32::from(item == instance)
    }
    /// `-1` when the argument is not an instance symbol.
    fn hlp_getinstanceid(&mut self, instance: i32) -> i32 {
        self.symbol_table
            .instance(instance)
            .map_or(-1, |instance| instance.value())
    }
    /// Random integer in `[0, bound)`, deterministic.
    fn hlp_random(&mut self, bound: i32) -> i32 {
        let bound = bound.max(1) as u32;
        (self.rng.next() % bound) as i32
    }
    /// Resolves an instance symbol to its npc id, scripts use this to reference
    /// other npcs. `0` when the argument is not an instance symbol (the value
    /// downstream `npc_*` host calls treat as "no npc").
    fn hlp_getnpc(&mut self, instance_symbol: i32) -> i32 {
        let Some(npc) = self.symbol_table.instance(instance_symbol) else {
            return 0;
        };
        npc.value()
    }
    /// The viewer has no separate npc handle table, an npc argument is valid when
    /// it resolves to an instance symbol (the resolution every other `npc_*` host
    /// call performs).
    fn hlp_isvalidnpc(&mut self, npc: i32) -> i32 {
        i32::from(self.symbol_table.instance(npc).is_some())
    }
    /// An item argument is valid when it resolves to a `C_Item` class instance.
    /// The viewer has no spawned item handles, inventory items are stored by
    /// their class instance id, so the class kind is the only validity signal.
    fn hlp_isvaliditem(&mut self, item: i32) -> i32 {
        let Some(item) = self.symbol_table.instance(item) else {
            return 0;
        };
        i32::from(self.typed.class_of(item.value() as u32) == Some(daedalus_typed::ClassKind::Item))
    }
    fake_impl!(hlp_cutsceneplayed(_0) -> i32);
}
