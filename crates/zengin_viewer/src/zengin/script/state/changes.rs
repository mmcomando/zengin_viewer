use bevy::prelude::{Entity, Message, Transform};

use crate::zengin::{ItemHandle, NpcHandle};

use super::super::string_value::StringCanon;
use super::super::symbol_id::{InstanceId, StringId};
use super::weapons::OverlayRequest;

/// Signal that an npc requested `Npc_ExchangeRoutine`: its routine entries are
/// replaced by the ones created by the `Rtn_{name}_{npcId}` function.
#[derive(Message)]
pub struct RoutineExchange {
    pub instance_id: InstanceId,
    /// Routine name with its original case (symbol names compare exactly).
    pub routine_name: String,
}

/// Signal that an item's shared data changed.
#[derive(Debug, Message, Clone, Copy)]
pub struct ItemChanged {
    pub instance_id: InstanceId,
}

/// Signal that the entities carrying an item instance need to refresh their
/// mirrored components.
#[derive(Debug, Message, Clone)]
pub struct ItemEntitiesChanged {
    /// Shared data of the instance, so consumers do not need a second lookup.
    pub handle: ItemHandle,
    pub entities: Vec<Entity>,
}

/// Signal that an npc's shared data changed; consumers compare the fields they
/// mirror and update their component only when the value actually differs.
#[derive(Debug, Message, Clone, Copy)]
pub struct NpcChanged {
    pub instance_id: InstanceId,
}

/// Signal that the entities carrying an npc instance need to refresh their
/// mirrored components.
#[derive(Debug, Message, Clone)]
pub struct NpcEntitiesChanged {
    /// Shared data of the instance, so consumers do not need a second lookup.
    pub handle: NpcHandle,
    pub entities: Vec<Entity>,
}

/// Signal that an instance requested `AI_GoToWP` (walk to a waypoint).
#[derive(Message)]
pub struct GotoChange {
    pub instance_id: InstanceId,
    /// Waypoint name string symbol id.
    pub waypoint: i32,
}

/// Signal that an instance requested an overlay change
/// (`Mdl_ApplyOverlayMds`/`Mdl_ApplyOverlayMdsTimed`/`Mdl_RemoveOverlayMds`).
#[derive(Message)]
pub struct OverlayChange {
    pub instance_id: InstanceId,
    pub request: OverlayRequest,
}

/// Signal that an instance requested `AI_AlignToWP` (face its routine waypoint).
#[derive(Message)]
pub struct AlignToWpChange {
    pub instance_id: InstanceId,
}

/// Signal that an instance requested `AI_Attack` (attack its current enemy).
#[derive(Message)]
pub struct AttackChange {
    pub instance_id: InstanceId,
    /// Target set via `Npc_SetTarget`, the entity to attack.
    pub target: InstanceId,
}

/// Signal that an instance requested `Npc_ClearAIQueue` (drop all queued npc
/// actions).
#[derive(Message)]
pub struct ClearAiQueueChange {
    pub instance_id: InstanceId,
}

/// Requests an item entity to be spawned, with its resolved world placement.
#[derive(Message)]
pub struct SpawnItem {
    pub instance_id: InstanceId,
    pub location_name: Option<StringId>, // Waypoint or spot name
    pub transform: Option<Transform>,
}

/// Requests an npc entity to be spawned.
#[derive(Message)]
pub struct SpawnNpc {
    pub instance_id: InstanceId,
    pub location_name: Option<StringId>, // Waypoint or spot name
    pub transform: Option<Transform>,
    /// Use the transform verbatim instead of the ground-placement offset and
    /// model-scale override. Used when restoring a saved world.
    pub exact_transform: bool,
    /// Overlay animation scheme to apply at spawn (saved world restore).
    pub overlay: Option<String>,
}

/// Items an npc's inventory gained through the script, recorded only when the
/// receiver is the player.
#[derive(Debug, Message)]
pub struct InventoryGain {
    pub npc: InstanceId,
    pub item: InstanceId,
    pub count: i32,
}

/// Signal that the script asked to end the dialog (`AI_StopProcessInfos`).
#[derive(Debug, Message)]
pub struct DialogClose;

/// On-screen message requested by `PrintScreen`/`AI_PrintScreen`. Shown for
/// `time_sec` seconds (the engine adds another second of fade-out), `x`/`y`
/// are per cent of the screen size, negative values center the text.
#[derive(Debug, Clone, Message)]
pub struct ScreenMessage {
    pub text: String,
    pub x: i32,
    pub y: i32,
    pub time_sec: i32,
}

#[derive(Debug, Clone, Message)]
pub struct DialogLine {
    pub npc_a: InstanceId,
    pub npc_b: InstanceId,
    pub line_id: StringId,
    /// Canonical (lowercased) name of the cutscene block referenced by the
    /// line, used to look up the real text and the sound file
    pub line_str: StringCanon,
}
