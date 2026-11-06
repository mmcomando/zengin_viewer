//! Shared item data handles: the script instance id plus the typed `C_Item`
//! record and the viewer runtime state, both behind their own locks.

use std::sync::{Arc, RwLock};

use daedalus_typed::Item;

use crate::zengin::script::state::ItemRuntime;
use crate::zengin::script::symbol_id::InstanceId;

/// Handle to one item instance. Cloning it is cheap: the data is shared through
/// the two `Arc`s, only the instance id is copied.
#[derive(Debug, Clone)]
#[allow(dead_code)] // fields and accessors are read by item consumers (next step)
pub struct ItemHandle {
    instance_id: InstanceId,
    typed: Arc<RwLock<Item>>,
    runtime: Arc<RwLock<ItemRuntime>>,
}

#[allow(dead_code)] // accessors are read by item consumers (next step)
impl ItemHandle {
    pub fn new(
        instance_id: InstanceId,
        typed: Arc<RwLock<Item>>,
        runtime: Arc<RwLock<ItemRuntime>>,
    ) -> Self {
        Self {
            instance_id,
            typed,
            runtime,
        }
    }

    pub fn instance_id(&self) -> InstanceId {
        self.instance_id
    }

    pub fn typed(&self) -> &Arc<RwLock<Item>> {
        &self.typed
    }

    pub fn runtime(&self) -> &Arc<RwLock<ItemRuntime>> {
        &self.runtime
    }
}
