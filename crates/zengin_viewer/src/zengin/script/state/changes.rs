use bevy::prelude::{Entity, Message, Transform, Vec3};

use crate::zengin::{ItemHandle, NpcHandle};

use super::super::string_value::StringCanon;
use super::super::symbol_id::{FunctionId, InstanceId, StringId};
use super::weapons::OverlayRequest;

/// Signal that an npc requested `Npc_ExchangeRoutine`: its routine entries are
/// replaced by the ones created by the `Rtn_{name}_{npcId}` function.
#[derive(Message)]
pub struct RoutineExchange {
    pub instance_id: InstanceId,
    /// Routine name with its original case (symbol names compare exactly).
    pub routine_name: String,
}

/// Signal that a script requested `Wld_SetTime`: move the world clock to the
/// next occurrence of `hour:minute`. Hours past 24 are used by the scripts
/// (`Wld_SetTime(24,0)` means the next midnight).
#[derive(Debug, Clone, Copy, Message)]
pub struct SetGameTime {
    pub hour: i32,
    pub minute: i32,
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

/// Signal that an npc ran `AI_UnEquipWeapons`. The debug trace logs it even
/// when the resulting weapon mode did not change.
#[derive(Debug, Message, Clone, Copy)]
pub struct NpcWeaponsUnequipped {
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

/// Signal that a script requested an animation for an npc
/// (`AI_PlayAni`/`AI_PlayAniBS`). Later requests from the same script call
/// override earlier ones.
#[derive(Debug, Clone, Message)]
pub struct AnimationChange {
    pub instance_id: InstanceId,
    pub animation: StringCanon,
}

/// Signal that a script requested `AI_UseMob`: an npc should use a mob with
/// the given `AI_UseMob` scheme, `state` > 0 attaches and `state` <= 0
/// releases.
#[derive(Debug, Clone, Message)]
pub struct UseMobChange {
    pub instance_id: InstanceId,
    pub mob_name: StringCanon,
    pub state: i32,
}

/// Signal that a script requested `AI_UseItemToState`: an npc should hold/use
/// `item` in the item-state animation identified by `scheme`, `state` >= 0
/// enters that state and `state` < 0 releases it.
#[derive(Debug, Clone, Message)]
pub struct UseItemStateChange {
    pub instance_id: InstanceId,
    /// Item instance symbol used by the npc.
    pub item: InstanceId,
    /// Item `scemename`, e.g. `LUTE` or `FOOD`, used as the animation scheme.
    pub scheme: StringCanon,
    pub state: i32,
}

/// Signal that a script requested `AI_WaitMS`: the npc should idle for the
/// given time before the next queued action starts.
#[derive(Debug, Clone, Copy, Message)]
pub struct WaitChange {
    pub instance_id: InstanceId,
    pub milliseconds: i32,
}

/// Signal that a script requested `AI_StandUp` (or `AI_StandUpQuick`): the npc
/// should stop what it is doing and stand up idle, releasing any mob use.
#[derive(Debug, Clone, Message)]
pub struct StandUpChange {
    pub instance_id: InstanceId,
}

/// Signal that a script started an AI state (`AI_StartState`). The state
/// replaces the running routine state and is not replaced by routine entry
/// changes while it runs.
#[derive(Debug, Clone, Message)]
pub struct StartStateChange {
    pub instance_id: InstanceId,
    pub state: FunctionId,
    /// Instance the state starts against (the script `other` global, else the
    /// npc target).
    pub other: Option<InstanceId>,
}

/// Signal that an instance requested `AI_GoToWP` (walk to a waypoint).
#[derive(Message)]
pub struct GotoChange {
    pub instance_id: InstanceId,
    /// Waypoint name string symbol id.
    pub waypoint: i32,
}

/// Signal that an instance should walk to a freepoint resolved by the script
/// host (`AI_GotoFP`/`AI_GotoNextFP`).
#[derive(Debug, Clone, Message)]
pub struct GotoFpChange {
    pub instance_id: InstanceId,
    /// Lowercased name of the chosen freepoint.
    pub point_name: StringCanon,
}

/// Signal that an instance requested `AI_GotoNpc` (walk to another npc).
#[derive(Debug, Clone, Copy, Message)]
pub struct GotoNpcChange {
    pub instance_id: InstanceId,
    pub target: InstanceId,
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

/// Signal that an instance requested `AI_TurnToNPC` (turn to face another npc).
#[derive(Debug, Clone, Message)]
pub struct TurnToNpcChange {
    pub instance_id: InstanceId,
    pub target: InstanceId,
}

/// Signal that an instance requested `AI_ProcessInfos`: open the dialog of `npc`
/// with `other` as the talk partner.
#[derive(Debug, Clone, Message)]
pub struct ProcessInfosChange {
    pub npc: InstanceId,
    pub other: InstanceId,
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

/// Chapter introduction requested by `IntroduceChapter`: a splash screen with
/// a background texture, a title and a subtitle, shown for `time_ms`
/// milliseconds (`<= 0` keeps it until the player closes it).
///
/// Texture and sound are the raw short names from the script (for example
/// `chapter1.tga` and `chapter_01.wav`), resolved against the vfs by the GUI.
#[derive(Debug, Clone, Message)]
pub struct ChapterIntro {
    pub title: String,
    pub subtitle: String,
    pub texture: String,
    pub sound: String,
    pub time_ms: i32,
}

/// Perception broadcast requested by `Npc_SendPassivePerc`: nearby npcs sense
/// `source` (position/range) with `other` and `victim` bound for their
/// registered callback of `perception`.
#[derive(Debug, Clone, Copy, Message)]
pub struct PassivePerception {
    pub source: InstanceId,
    pub other: InstanceId,
    pub victim: Option<InstanceId>,
    pub perception: i32,
}

/// Speech line requested by `AI_OutputSVM`/`AI_OutputSVM_Overlay`. `sound` is
/// the `C_SVM` file name (e.g. `SVM_1_IGetYouStill`) resolved for the speaking
/// npc voice, `position` is its world position in meters.
#[derive(Debug, Clone, Message)]
pub struct SvmLine {
    pub speaker: InstanceId,
    pub sound: String,
    pub position: Option<Vec3>,
    /// `AI_OutputSVM_Overlay`: the engine blocks further overlay lines from the
    /// same speaker while the previous one plays.
    pub overlay: bool,
}

/// Sound requested by `Snd_Play`/`Snd_Play3D` and the `AI_Snd_Play*` variants.
/// The name is the uppercased `C_SFX` instance name, the position is the world
/// position in meters for the 3D variants and `None` for global playback.
#[derive(Debug, Clone, Message)]
pub struct ScriptSound {
    pub name: String,
    pub position: Option<Vec3>,
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
