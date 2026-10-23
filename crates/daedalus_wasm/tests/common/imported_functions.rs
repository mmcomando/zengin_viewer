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
    /// Contents of the static string pairs, loaded before running a script
    /// (`call_get_name` for ids `0..strings_num`), used to resolve string
    /// arguments of the string-returning externals.
    pub static_strings: Vec<String>,
    /// Number of static string pairs, dynamic string ids start here
    pub strings_num: i32,
    /// Mirror of the guest DYN_STRINGS ring: raw content the host returned for
    /// each dynamic string slot, indexed by `slot = id - strings_num`. Filled
    /// in call order which matches the guest ring cursor.
    pub dynamic_strings: Vec<Vec<u8>>,
    /// Recorded string-returning external calls for verification, see the
    /// `real_printdebugnpc_dyn_strings` test.
    pub string_calls: Vec<StringCall>,
    /// Arguments of `printdebuginstch` calls as (npc, string id)
    pub print_debug_inst_ch_calls: Vec<(i32, i32)>,
}

/// One string-returning external call, `slot` is the DYN_STRINGS ring slot the
/// guest will copy the returned string into
#[derive(Debug, Clone)]
pub struct StringCall {
    pub name: &'static str,
    pub args: Vec<i32>,
    pub slot: usize,
}

impl ImportCallState {
    /// Records a string-returning external call and returns the ring slot its
    /// result will be copied into, which mirrors the guest ring cursor
    pub fn record_string_call(&mut self, name: &'static str, args: Vec<i32>) -> usize {
        let slot = self.dynamic_strings.len();
        self.string_calls.push(StringCall { name, args, slot });
        slot
    }

    /// Stores the content of a returned string in the ring mirror
    pub fn store_dynamic_string(&mut self, slot: usize, content: Vec<u8>) {
        if slot >= self.dynamic_strings.len() {
            self.dynamic_strings.push(content);
        } else {
            self.dynamic_strings[slot] = content;
        }
    }

    /// Content of a string id, static ids come from the preloaded table,
    /// dynamic ids from the ring mirror
    pub fn string_content(&self, raw: i32) -> Vec<u8> {
        if raw < self.strings_num {
            return self
                .static_strings
                .get(raw.max(0) as usize)
                .cloned()
                .unwrap_or_default()
                .into_bytes();
        }
        let slot = (raw - self.strings_num) as usize;
        self.dynamic_strings.get(slot).cloned().unwrap_or_default()
    }
}
