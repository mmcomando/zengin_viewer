use std::collections::HashSet;

use bevy::{
    math::Vec3,
    prelude::{Component, Message},
};

use super::super::inventory::Inventory;
use super::super::string_value::StringCanon;
use super::super::symbol_id::{FunctionId, InstanceId, StringId};
use super::weapons::WeaponMode;
#[derive(Debug, Default, Clone, PartialEq, Component)]
pub struct RoutineEntry {
    pub start_h: u32,
    pub start_m: u32,
    pub stop_h: u32,
    pub stop_m: u32,
    /// Canonical (lowercased) waypoint name the entry points at
    pub way_point: StringCanon,
    /// Symbol of the routine state function (`ZS_*`), `None` when the state
    /// has no function.
    pub state_fn: Option<FunctionId>,
}

/// Runtime state of an npc instance, grouped by domain.
#[derive(Debug, Default)]
pub struct NpcRuntime {
    pub visual: NpcVisualState,
    pub routine: NpcRoutineState,
    pub equipment: NpcEquipmentState,
    pub dialog: NpcDialogState,
    /// Item instance counts.
    pub inventory: Inventory,
    pub talents: NpcTalents,
    pub social: NpcSocialState,
    pub live: NpcLiveState,
}

/// Visual state set by the model functions and resolved at load time.
#[derive(Debug, Default)]
pub struct NpcVisualState {
    /// Set when `Mdl_SetVisualBody`/`Mdl_SetVisual` ran during instance init.
    /// Only these instances are treated as npcs.
    pub initialized: bool,
    /// Body model symbol set via `Mdl_SetVisualBody`.
    pub body_model_id: Option<StringId>,
    pub head_model_id: Option<StringId>,
    /// Hierarchy (visual `.mds` stem) set via `Mdl_SetVisual`.
    pub hierarchy_id: Option<StringId>,
    /// Texture variant index, `0` means default.
    pub body_texture_index: i32,
    /// Texture variant index, `0` means default.
    pub face_texture_index: i32,

    // -- visual body, resolved strings --
    pub body_model: String,
    pub body_texture: Option<String>,
    pub head_model: Option<String>,
    pub face_texture: Option<String>,
    pub hierarchy: Option<String>,

    /// Model scale set via `Mdl_SetModelScale`.
    pub model_scale: Option<Vec3>,

    /// Body fatness set via `Mdl_SetModelFatness`.
    pub model_fatness: Option<f32>,
}

/// Routine and AI-state data of an npc.
#[derive(Debug, Default)]
pub struct NpcRoutineState {
    /// Routine entries set via `TA_Min`.
    pub entries: Vec<RoutineEntry>,
    /// Animations requested via `AI_PlayAni(BS)` by the last executed routine
    /// state function, in request order.
    pub played_anis: Vec<StringCanon>,
    /// State time (seconds) set via `NPC_SetStateTime`.
    pub state_time: i32,
    /// Routine state function currently active (`Npc_IsInState`).
    pub current_state: Option<FunctionId>,
    /// Previously active routine state function (`Npc_WasInState`).
    pub previous_state: Option<FunctionId>,
}

/// Equipped items and weapon mode of an npc.
#[derive(Debug, Default)]
pub struct NpcEquipmentState {
    /// Melee weapon equipped (`EquipItem` with an `itmw_*` item).
    pub melee: Option<InstanceId>,
    /// Ranged weapon equipped (`EquipItem` with an `itrw_*` item).
    pub ranged: Option<InstanceId>,
    /// Armor equipped (`EquipItem` with an `itar_*` item).
    pub armor: Option<InstanceId>,
    /// Weapon mode (`Npc_SetToFightMode`/`SetToFistMode`/weapon AI).
    pub weapon_mode: Option<WeaponMode>,
}

/// Dialog knowledge of an npc.
#[derive(Debug, Default)]
pub struct NpcDialogState {
    /// Root dialog options (`C_INFO`) of this npc, `None` until dialog
    /// information is loaded for it.
    pub roots: Option<Vec<DialogRoot>>,
    /// Info (`C_INFO`) instances known by this npc (`Npc_KnowsInfo`).
    pub known_infos: HashSet<InstanceId>,
}

/// Attitude values of an npc.
#[derive(Debug, Default)]
pub struct NpcSocialState {
    /// Permanent attitude (`Npc_SetAttitude`).
    pub attitude: Option<i32>,
    /// Temporary attitude (`Npc_SetTempAttitude`).
    pub temp_attitude: Option<i32>,
}

/// Live world facts shared with the ECS side.
#[derive(Debug, Default)]
pub struct NpcLiveState {
    /// World position, `None` when the npc has no entity (unknown or dead).
    pub position: Option<Vec3>,
    /// Npc currently looked at (`AI_LookAtNPC`/`AI_StopLookAt`).
    pub look_at: Option<InstanceId>,
}

impl NpcRuntime {
    /// Routine entry active at `minutes_of_day`, matching the engine semantics:
    /// entries start (inclusive) at their start time and stop (exclusive) at
    /// their stop time, entries wrapping over midnight stay active past it.
    pub fn get_routine_entry(&self, minutes_of_day: u32) -> Option<&RoutineEntry> {
        let first = self.routine.entries.first()?;
        let minutes = minutes_of_day % (24 * 60);
        for entry in &self.routine.entries {
            let start = entry.start_h * 60 + entry.start_m;
            let stop = entry.stop_h * 60 + entry.stop_m;
            let in_range = if stop <= start {
                minutes >= start || minutes < stop
            } else {
                minutes >= start && minutes < stop
            };

            if in_range {
                return Some(entry);
            }
        }

        return Some(first);
    }
}

/// Runtime state of an item instance, keyed by instance id in
/// [`LoadedState::items_instances`].
#[derive(Debug, Default)]
pub struct ItemRuntime {
    /// Instance (symbol) name.
    pub name: String,
    /// Visual set on the item instance.
    pub visual: String,
    /// Visual override (`visual_change`), takes precedence when set.
    pub visual_change: Option<String>,
}

impl ItemRuntime {
    /// Visual used for rendering, the override when set.
    pub fn chosen_visual(&self) -> &str {
        self.visual_change.as_deref().unwrap_or(&self.visual)
    }
}

#[derive(Debug, Clone, Message)]
pub struct DialogRoot {
    /// Symbol of the `C_INFO` instance this root was loaded from
    pub info_instance_id: Option<InstanceId>,
    pub sequence_function_id: FunctionId,
    /// `None` when the root has no condition function
    pub condition_function_id: Option<FunctionId>,
    pub order: i32,
    pub description: String,
    pub important: bool,
    pub permanent: bool,
    /// Selecting this root starts a trade with the npc after its lines play.
    pub trade: bool,
}

/// A sub dialog choice registered by `Info_AddChoice` for a `C_INFO` instance.
#[derive(Debug, Clone)]
pub struct DialogChoice {
    /// Choice text shown in the menu.
    pub description: String,
    pub sequence_function_id: FunctionId,
}

/// Talent slots per npc, `TALENT_MAX_G2`.
pub const TALENTS_MAX: usize = 22;

/// Talent skill and value slots of one npc.
#[derive(Debug, Default)]
pub struct NpcTalents {
    skills: [i32; TALENTS_MAX],
    values: [i32; TALENTS_MAX],
}

impl NpcTalents {
    fn slot(talent: i32, slots: &[i32; TALENTS_MAX]) -> Option<i32> {
        usize::try_from(talent)
            .ok()
            .filter(|tt| *tt < TALENTS_MAX)
            .map(|tt| slots[tt])
    }

    fn slot_mut(talent: i32, slots: &mut [i32; TALENTS_MAX]) -> Option<&mut i32> {
        usize::try_from(talent)
            .ok()
            .filter(|tt| *tt < TALENTS_MAX)
            .map(|tt| &mut slots[tt])
    }

    pub fn skill(&self, talent: i32) -> i32 {
        Self::slot(talent, &self.skills).unwrap_or(0)
    }

    pub fn value(&self, talent: i32) -> i32 {
        Self::slot(talent, &self.values).unwrap_or(0)
    }

    pub fn set_skill(&mut self, talent: i32, level: i32) {
        if let Some(slot) = Self::slot_mut(talent, &mut self.skills) {
            *slot = level;
        }
    }

    pub fn set_value(&mut self, talent: i32, level: i32) {
        if let Some(slot) = Self::slot_mut(talent, &mut self.values) {
            *slot = level;
        }
    }
}
