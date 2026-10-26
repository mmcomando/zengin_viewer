use bevy::prelude::{Message, Transform};

use super::super::string_value::StringCanon;
use super::super::symbol_id::{InstanceId, StringId};
use super::weapons::OverlayRequest;

/// Signal that an instance's routine entries changed (a `TA_Min` was recorded).
#[derive(Message)]
pub struct RoutineEntryChange {
    pub instance_id: InstanceId,
}

/// Signal that an npc requested `Npc_ExchangeRoutine`: its routine entries are
/// replaced by the ones created by the `Rtn_{name}_{npcId}` function.
#[derive(Message)]
pub struct RoutineExchange {
    pub instance_id: InstanceId,
    /// Routine name with its original case (symbol names compare exactly).
    pub routine_name: String,
}

/// Signal that an instance's `AI_LookAtNPC`/`AI_StopLookAt` target changed.
#[derive(Message)]
pub struct NpcLookAtChange {
    pub instance_id: InstanceId,
}

/// Signal that an instance's weapon mode changed.
#[derive(Message)]
pub struct WeaponModeChange {
    pub instance_id: InstanceId,
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

/// Signal that an instance equipped an item (`EquipItem`).
#[derive(Message)]
pub struct EquipItemChange {
    pub instance_id: InstanceId,
}

/// Signal that an instance hitpoints changed.
#[derive(Message)]
pub struct HealthChange {
    pub instance_id: InstanceId,
}

/// Signal that an instance requested `AI_AlignToWP` (face its routine waypoint).
#[derive(Message)]
pub struct AlignToWpChange {
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
