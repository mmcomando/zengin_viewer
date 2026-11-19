use super::*;

impl mmcomando::zengine_scripts::mob_scripts::Host for ImportCallState {
    fake_impl!(mob_createitems(_0, _1, _2));
    /// Count of `item` in the container vob named `mob_name`, `0` when either is
    /// unknown. The engine looks the container up by its vob name; unnamed
    /// chests cannot be addressed this way.
    fn mob_hasitems(&mut self, mob_name: i32, item: i32) -> i32 {
        let Some(mob_name) = self.symbol_table.string(mob_name) else {
            return 0;
        };
        let Some(mob_name) = self.get_canon_string(mob_name) else {
            return 0;
        };
        let Some(item) = self.symbol_table.instance(item) else {
            return 0;
        };
        self.container_item_count(mob_name.as_str(), item)
    }
}
