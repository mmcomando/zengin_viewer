use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct ImportCallState {
    pub call_counts: HashMap<String, u64>,
}
