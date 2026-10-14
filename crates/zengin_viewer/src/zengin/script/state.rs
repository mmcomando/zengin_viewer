use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use bevy::math::Vec3;

use super::bindings::MmWorld;
use super::bindings::exports::mmcomando::zengine_scripts::manage::SymbolKind;
use super::symbol_id::{FunctionId, InstanceId, StringId, SymbolId};

#[derive(Debug, Default)]
pub struct RoutineEntry {
    pub start_h: u32,
    pub start_m: u32,
    pub stop_h: u32,
    pub stop_m: u32,
    pub way_point: String,
    /// Symbol of the routine state function (`ZS_*`), `None` when the state
    /// has no function.
    pub state_fn: Option<FunctionId>,
}

#[derive(Debug, Default)]
pub struct InstanceState {
    pub body_model: String,
    pub body_texture: Option<String>,
    pub head_model: Option<String>,
    pub face_texture: Option<String>,
    pub armor_model: Option<String>,
    pub hierarchy: Option<String>,

    /// Model scale set via `Mdl_SetModelScale`
    pub model_scale: Option<Vec3>,

    pub routine_enties: Vec<RoutineEntry>,
}

impl InstanceState {
    /// Routine entry active at `minutes_of_day`, matching the engine semantics:
    /// entries start (inclusive) at their start time and stop (exclusive) at
    /// their stop time, entries wrapping over midnight stay active past it.
    pub fn get_routine_entry(&self, minutes_of_day: u32) -> Option<&RoutineEntry> {
        let first = self.routine_enties.first()?;
        let minutes = minutes_of_day % (24 * 60);
        for entry in &self.routine_enties {
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

#[derive(Debug)]
pub struct SpawnNpc {
    pub npc_index: InstanceId,
    pub way_point: String,
}

#[derive(Debug, Default)]
pub struct SpawnItem {
    pub instance_name: String,
    pub way_point: String,
}

#[derive(Debug, Default)]
pub struct ItemInstance {
    pub visual: String,
    pub visual_change: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DialogRoot {
    /// Symbol of the `C_INFO` instance this root was loaded from, `None` for
    /// sub dialog choices registered via `Info_AddChoice`
    // TODO make Info_AddChoice a separate type
    pub info_instance_id: Option<InstanceId>,
    pub sequence_function_id: FunctionId,
    /// `None` when the root has no condition function
    pub condition_function_id: Option<FunctionId>,
    pub order: i32,
    pub description: String,
    pub important: bool,
    pub permanent: bool,
}

#[derive(Debug, Default)]
pub struct LoadedState {
    pub spawn_npcs: Vec<SpawnNpc>,
    pub spawn_items: Vec<SpawnItem>,
    pub instance_data: HashMap<InstanceId, InstanceState>,
    pub item_instances: HashMap<String, ItemInstance>,
    pub dialog_roots: HashMap<InstanceId, Vec<DialogRoot>>,
    /// Animations requested via `AI_PlayAni(BS)` by the last executed routine
    /// state function, keyed by npc instance index, in request order.
    pub routine_played_anis: HashMap<InstanceId, Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct DialogLine {
    pub npc_a: InstanceId,
    pub npc_b: InstanceId,
    pub line_id: StringId,
    pub line_str: String,
}

#[derive(Debug, Default)]
pub struct DialogState {
    pub dialog_lines: Vec<DialogLine>,
    pub close_dialog: bool,
    pub sub_dialog_choices: Vec<DialogRoot>,
}

#[derive(Debug, Default, Clone)]
pub struct InstanceStateRaw {
    pub body_model: Option<StringId>,
    /// Texture variant index (not a symbol id), `0` means default
    pub body_texture: i32,
    pub head_model: Option<StringId>,
    /// Texture variant index (not a symbol id), `0` means default
    pub face_texture: i32,
    /// Armor is an item instance
    pub armor_model: Option<SymbolId>,
    pub hierarchy: Option<StringId>,
}

/// Kind of every loaded symbol, keyed by symbol id. Used to verify typed
/// symbol ids ([`InstanceId`], [`FunctionId`], [`StringId`]) at conversion
/// points instead of trusting raw ids.
#[derive(Debug, Default)]
pub struct SymbolTable {
    kinds: HashMap<i32, SymbolKind>,
    /// Number of symbols, static string ids start after the last symbol id
    symbols_count: i32,
}

impl SymbolTable {
    pub fn new(kinds: HashMap<i32, SymbolKind>, symbols_count: i32) -> Self {
        Self {
            kinds,
            symbols_count,
        }
    }

    fn verified(&self, raw: i32, expected: SymbolKind) -> Option<SymbolId> {
        match self.kinds.get(&raw) {
            Some(kind) if *kind == expected => SymbolId::new(raw),
            _ => None,
        }
    }

    pub fn instance(&self, raw: i32) -> Option<InstanceId> {
        self.verified(raw, SymbolKind::Instance)
            .map(InstanceId::from_symbol)
    }

    /// Daedalus callables are not only `function` symbols: instance and
    /// prototype symbols carry executable code too (e.g. `init_newworld`
    /// or dialog info functions are instances)
    pub fn function(&self, raw: i32) -> Option<FunctionId> {
        match self.kinds.get(&raw) {
            Some(SymbolKind::Function | SymbolKind::Instance | SymbolKind::Prototype) => {
                SymbolId::new(raw).map(FunctionId::from_symbol)
            }
            _ => None,
        }
    }

    /// String literals are not symbols, their ids are indices into the static
    /// strings table placed after all symbol ids
    pub fn string(&self, raw: i32) -> Option<StringId> {
        if raw < self.symbols_count {
            return None;
        }
        SymbolId::new(raw).map(StringId::from_symbol)
    }
}

#[derive(Default)]
pub struct ImportCallState {
    pub world: Option<Arc<MmWorld>>,
    pub loaded_state: LoadedState,
    pub dialog_state: DialogState,
    /// Kind of every symbol, used to verify typed symbol ids
    pub symbol_table: SymbolTable,
    pub raw_instances: HashMap<InstanceId, InstanceStateRaw>,
    /// State time (seconds) set via `NPC_SetStateTime`, per npc instance.
    pub state_times: HashMap<InstanceId, i32>,
    /// Info (`C_INFO`) instances known by each npc instance.
    pub known_infos: HashMap<InstanceId, HashSet<InstanceId>>,
    /// Generator backing `Hlp_Random`.
    pub rng: ScriptRandom,
    /// Talent skills/values per npc instance (`Npc_SetTalentSkill`).
    pub talents: HashMap<InstanceId, NpcTalents>,
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
            .filter(|t| *t < TALENTS_MAX)
            .map(|t| slots[t])
    }

    fn slot_mut(talent: i32, slots: &mut [i32; TALENTS_MAX]) -> Option<&mut i32> {
        usize::try_from(talent)
            .ok()
            .filter(|t| *t < TALENTS_MAX)
            .map(|t| &mut slots[t])
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

/// Deterministic pseudo random generator backing `Hlp_Random`.
#[derive(Debug)]
pub struct ScriptRandom {
    state: u32,
}

impl Default for ScriptRandom {
    fn default() -> Self {
        Self { state: 0x9E37_79B9 }
    }
}

impl ScriptRandom {
    /// Advances the generator and returns the next raw value.
    pub fn next(&mut self) -> u32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.state = x;
        x
    }
}
