//! Components of the entities spawned from the loaded world data or its
//! scripts.

use bevy::prelude::*;

use crate::zengin::item_data::ItemHandle;
use crate::zengin::npc_data::NpcHandle;
use crate::zengin::script::symbol_id::InstanceId;

/// Entity spawned from the loaded world data or its scripts.
///
/// GUI, camera and other application-lifetime entities are not tagged, so
/// despawning this component's entities keeps them alive across world restarts.
#[derive(Component, Default)]
pub(crate) struct WorldEntityMarker;

/// The mesh of the whole current level.
#[derive(Component, Default)]
// #[require(WorldEntityMarker)]
pub struct WorldMesh {}

/// Static vob entity. Its vob id lives in the sibling `MobBase`.
#[derive(Component, Default)]
#[require(WorldEntityMarker)]
pub struct StaticMesh;

/// The player entity.
#[derive(Default, Component)]
#[require(WorldEntityMarker)]
pub struct PlayerCharacter;

/// Npc.
#[derive(Debug, Component, Clone)]
#[require(WorldEntityMarker)]
pub struct GameNpc {
    handle: NpcHandle,
}

impl GameNpc {
    pub fn new(handle: NpcHandle) -> Self {
        Self { handle }
    }

    /// Script instance id of this npc.
    pub fn instance_id(&self) -> InstanceId {
        self.handle.instance_id()
    }

    /// Shared typed and runtime data of this npc.
    pub fn handle(&self) -> &NpcHandle {
        &self.handle
    }
}

/// Identifies Entity with the shared data of a Zengin script item instance.
#[derive(Debug, Component, Clone)]
#[require(WorldEntityMarker)]
pub struct GameItem {
    handle: ItemHandle,
}

impl GameItem {
    pub fn new(handle: ItemHandle) -> Self {
        Self { handle }
    }

    /// Script instance id of this item.
    pub fn instance_id(&self) -> InstanceId {
        self.handle.instance_id()
    }

    /// Shared typed and runtime data of this item.
    pub fn handle(&self) -> &ItemHandle {
        &self.handle
    }
}

/// Interactive mob vob (`oCMobInter` and its subtypes).
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Component, Reflect)]
#[require(WorldEntityMarker)]
pub struct GameMob {
    pub use_with_item: String,
    pub on_state_func: String,
    pub condition_func: String,
    pub rewind: bool,
    /// Script name of the vob notified when the mob is used.
    pub trigger_target: String,
    /// Whether `trigger_target` was already notified. A mob with `rewind ==
    /// false` notifies it only once.
    pub trigger_fired: bool,
    /// Switch-like vob (lever, button, wheel, door): using it toggles the state
    /// and the player stands up automatically after the use animation.
    pub auto_end: bool,
}

/// Chest vob (`oCMobContainer`) with its remaining loot.
#[derive(Debug, Component)]
#[require(WorldEntityMarker)]
pub struct GameChest {
    /// Vob `locked` flag as loaded. A key or pick string is required to unlock.
    pub locked: bool,
    /// Script instance name of the unlocking item, empty when none.
    pub key: String,
    /// Lockpick combination, one `R`/`L` per step, empty when not pickable.
    pub pick_string: String,
    /// Unlocked by a key or a completed lockpick sequence, persists in saves.
    pub cracked: bool,
    /// Remaining contents as `(item, count)` pairs.
    pub items: Vec<(InstanceId, i32)>,
}

/// Data of a flying arrow, used for the hit and cleanup.
#[derive(Component)]
#[require(WorldEntityMarker)]
pub struct Arrow {
    pub weapon: InstanceId,
    pub shooter: InstanceId,
    pub(crate) spawned_at: f32,
    /// Shot origin, contacts hit before the arrow travels `ARROW_MIN_HIT_DISTANCE`
    /// are ignored (the shooter's own hit volume).
    pub(crate) spawn_origin: Vec3,
}
