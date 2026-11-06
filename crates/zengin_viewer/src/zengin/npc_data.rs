//! Shared npc data handles: the script instance id plus the typed `C_Npc`
//! record and the viewer runtime state, both behind their own locks.

use std::sync::{Arc, RwLock};

use daedalus_typed::Npc;

use crate::zengin::script::state::NpcRuntime;
use crate::zengin::script::symbol_id::InstanceId;

/// Handle to one npc instance. Cloning it is cheap: the data is shared through
/// the two `Arc`s, only the instance id is copied.
#[derive(Debug, Clone)]
pub struct NpcHandle {
    instance_id: InstanceId,
    typed: Arc<RwLock<Npc>>,
    runtime: Arc<RwLock<NpcRuntime>>,
}

impl NpcHandle {
    pub fn new(
        instance_id: InstanceId,
        typed: Arc<RwLock<Npc>>,
        runtime: Arc<RwLock<NpcRuntime>>,
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

    pub fn typed(&self) -> &Arc<RwLock<Npc>> {
        &self.typed
    }

    pub fn runtime(&self) -> &Arc<RwLock<NpcRuntime>> {
        &self.runtime
    }
}
