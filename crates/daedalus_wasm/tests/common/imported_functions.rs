use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct ImportCallState {
    pub call_counts: HashMap<String, u64>,
    /// Arguments of `ai_playani` and `ai_playanibs` calls as (npc, ani string
    /// id, body state), in call order.
    pub play_ani_calls: Vec<(i32, i32, i32)>,
    /// Value returned by the `npc_hasitems` stub, `0` by default. Set to `1`
    /// to mimic the viewer host stub which always reports items present.
    pub npc_hasitems_return: i32,
}
