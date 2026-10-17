use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use bevy::{
    math::Vec3,
    prelude::{Component, Message, Transform},
};
use rand_mt::Mt;

use super::bindings::MmWorld;
use super::bindings::exports::mmcomando::zengine_scripts::manage::SymbolKind;
use super::string_value::{ShowString, StringCanon};
use super::symbol_id::{FunctionId, InstanceId, StringId, SymbolId};

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

/// Signal that an instance's routine entries changed (a `TA_Min` was recorded).
/// Produced by the script, drained to the ECS by the script bridge.
#[derive(Message)]
pub struct RoutineEntryChange {
    pub instance_id: InstanceId,
}

/// Signal that an instance's `AI_LookAtNPC`/`AI_StopLookAt` target changed.
/// Produced by the script, drained to the ECS by the script bridge.
#[derive(Message)]
pub struct NpcLookAtChange {
    pub instance_id: InstanceId,
}

/// Signal that an instance's weapon mode changed (`Npc_SetToFightMode`,
/// `Npc_SetToFistMode`, `AI_RemoveWeapon`, `AI_ReadyMeleeWeapon`). Produced by
/// the script, drained to the ECS by the script bridge.
#[derive(Message)]
pub struct WeaponModeChange {
    pub instance_id: InstanceId,
}

/// Signal that an instance requested `AI_GoToWP` (walk to a waypoint). Produced
/// by the script, drained to the ECS by the script bridge.
#[derive(Message)]
pub struct GotoChange {
    pub instance_id: InstanceId,
    /// Waypoint name string symbol id.
    pub waypoint: i32,
}

/// Signal that an instance requested an overlay change
/// (`Mdl_ApplyOverlayMds`/`Mdl_ApplyOverlayMdsTimed`/`Mdl_RemoveOverlayMds`).
/// Produced by the script, drained to the ECS by the script bridge.
#[derive(Message)]
pub struct OverlayChange {
    pub instance_id: InstanceId,
    pub request: OverlayRequest,
}

/// Signal that an instance equipped an item (`EquipItem`). Produced by the
/// script, drained to the ECS by the script bridge.
#[derive(Message)]
pub struct EquipItemChange {
    pub instance_id: InstanceId,
}

/// Signal that an instance requested `AI_AlignToWP` (face its routine waypoint).
/// Produced by the script, drained to the ECS by the script bridge.
#[derive(Message)]
pub struct AlignToWpChange {
    pub instance_id: InstanceId,
}

/// Requests an item entity to be spawned, with its resolved world placement.
/// Produced by the world load, drained to the ECS by the script bridge.
#[derive(Message)]
pub struct SpawnItem {
    pub instance_id: InstanceId,
    pub location_name: Option<StringId>, // Waypoint or spot name
    pub transform: Option<Transform>,
}

/// Requests an npc entity to be spawned. Produced by `wld_insertnpc` (at script
/// load and at runtime), drained to the ECS by the script bridge. The consumer
/// resolves the visual and placement from the script state.
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
/// Produced by the script, drained to the ECS by the script bridge.
#[derive(Debug, Message)]
pub struct DialogClose;
/// Runtime state of an npc instance.
#[derive(Debug, Default)]
pub struct NpcInstanceState {
    /// Set when `Mdl_SetVisualBody`/`Mdl_SetVisual` ran during instance init.
    /// Only these instances are treated as npcs.
    pub visual_initialized: bool,
    /// Body model symbol set via `Mdl_SetVisualBody`.
    pub body_model_id: Option<StringId>,
    pub head_model_id: Option<StringId>,
    /// Armor is an item instance.
    pub armor_model_id: Option<SymbolId>,
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
    pub armor_model: Option<String>,
    pub hierarchy: Option<String>,

    /// Model scale set via `Mdl_SetModelScale`.
    pub model_scale: Option<Vec3>,

    /// Body fatness set via `Mdl_SetModelFatness`.
    pub model_fatness: Option<f32>,

    /// Routine entries set via `TA_Min`.
    pub routine_entries: Vec<RoutineEntry>,

    /// Root dialog options (`C_INFO`) of this npc, `None` until dialog
    /// information is loaded for it.
    pub dialog_roots: Option<Vec<DialogRoot>>,

    /// Animations requested via `AI_PlayAni(BS)` by the last executed routine
    /// state function, in request order.
    pub routine_played_anis: Vec<StringCanon>,

    /// State time (seconds) set via `NPC_SetStateTime`.
    pub state_time: i32,

    /// Info (`C_INFO`) instances known by this npc (`Npc_KnowsInfo`).
    pub known_infos: HashSet<InstanceId>,

    /// Talent skills/values (`Npc_SetTalentSkill`/`Npc_SetTalentValue`).
    pub talents: NpcTalents,

    /// Inventory, item instance id to count.
    pub inventory: HashMap<InstanceId, i32>,

    /// Routine state function currently active (`Npc_IsInState`).
    pub current_routine_state: Option<FunctionId>,

    /// Previously active routine state function (`Npc_WasInState`).
    pub previous_routine_state: Option<FunctionId>,

    /// Permanent attitude (`Npc_SetAttitude`).
    pub attitude: Option<i32>,

    /// Temporary attitude (`Npc_SetTempAttitude`).
    pub temp_attitude: Option<i32>,

    // -- live facts shared with the ECS side --
    /// World position, `None` when the npc has no entity (unknown or dead).
    pub position: Option<Vec3>,

    /// Npc currently looked at (`AI_LookAtNPC`/`AI_StopLookAt`).
    pub look_at: Option<InstanceId>,

    /// Weapon mode (`Npc_SetToFightMode`/`SetToFistMode`/weapon AI).
    pub weapon_mode: Option<WeaponMode>,

    /// Item symbol equipped (`EquipItem`).
    pub equipped_weapon: Option<InstanceId>,
}

impl NpcInstanceState {
    /// Routine entry active at `minutes_of_day`, matching the engine semantics:
    /// entries start (inclusive) at their start time and stop (exclusive) at
    /// their stop time, entries wrapping over midnight stay active past it.
    pub fn get_routine_entry(&self, minutes_of_day: u32) -> Option<&RoutineEntry> {
        let first = self.routine_entries.first()?;
        let minutes = minutes_of_day % (24 * 60);
        for entry in &self.routine_entries {
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
pub struct ItemInstanceState {
    /// Instance (symbol) name.
    pub name: String,
    /// Visual set on the item instance.
    pub visual: String,
    /// Visual override (`visual_change`), takes precedence when set.
    pub visual_change: Option<String>,
}

impl ItemInstanceState {
    /// Visual used for rendering, the override when set.
    pub fn chosen_visual(&self) -> &str {
        self.visual_change.as_deref().unwrap_or(&self.visual)
    }
}

#[derive(Debug, Clone, Message)]
pub struct DialogRoot {
    /// Symbol of the `C_INFO` instance this root was loaded from, `None` for
    /// sub dialog choices registered via `Info_AddChoice`
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
    /// State of every loaded item instance.
    pub items_instances: HashMap<InstanceId, ItemInstanceState>,
    /// State of every loaded npc instance.
    pub npc_instances: HashMap<InstanceId, NpcInstanceState>,
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

/// Kind of every loaded symbol, keyed by symbol id. Used to verify typed
/// symbol ids ([`InstanceId`], [`FunctionId`], [`StringId`]) at conversion
/// points instead of trusting raw ids. Also holds the contents of static
/// string literals, loaded once at startup so import handlers never need to
/// call back into the guest to resolve string content.
#[derive(Debug, Default)]
pub struct SymbolTable {
    kinds: HashMap<i32, SymbolKind>,
    /// Number of symbols, static string ids start after the last symbol id
    symbols_count: i32,
    /// Contents of the guest's string pairs, indexed by raw id: ids below
    /// `symbols_count` hold symbol names, ids above hold static string
    /// literals. Loaded once at startup so import handlers never need to call
    /// back into the guest to resolve string content.
    strings: Vec<String>,
}

impl SymbolTable {
    pub fn new(kinds: HashMap<i32, SymbolKind>, symbols_count: i32, strings: Vec<String>) -> Self {
        Self {
            kinds,
            symbols_count,
            strings,
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
        StringId::new(raw)
    }

    /// Content of the string table entry `raw` (symbol name or static string
    /// literal), `None` when the id is out of range or the entry is empty.
    /// TODO Empty strings should be a proper value.
    /// Currently a lot of calls returns empty string which is probably a bug
    /// somewhere in scripts implementation.
    /// To prevent further "head model no found errors" return None here.
    pub fn string_content(&self, raw: i32) -> Option<String> {
        let string = self.strings.get(raw.max(0) as usize)?;
        if string.is_empty() {
            return None;
        }
        Some(string.clone())
    }
}

#[derive(Default)]
pub struct ImportCallState {
    pub world: Option<Arc<MmWorld>>,
    pub loaded_state: LoadedState,
    /// Set by `AI_StopProcessInfos`, taken by the script bridge to emit a
    /// [`DialogClose`] message.
    pub close_dialog: bool,
    /// Kind of every symbol, used to verify typed symbol ids
    pub symbol_table: SymbolTable,
    /// Routine entry changes produced by the script, drained to the ECS by the
    /// script bridge.
    pub pending_routine_changes: Vec<RoutineEntryChange>,
    /// Look-at target changes produced by the script, drained to the ECS by the
    /// script bridge.
    pub pending_look_at_changes: Vec<NpcLookAtChange>,
    /// Weapon mode changes produced by the script, drained to the ECS by the
    /// script bridge.
    pub pending_weapon_mode_changes: Vec<WeaponModeChange>,
    /// Equipped item changes produced by the script, drained to the ECS by the
    /// script bridge.
    pub pending_equip_item_changes: Vec<EquipItemChange>,
    /// `AI_AlignToWP` requests produced by the script, drained to the ECS by the
    /// script bridge.
    pub pending_align_to_wp_changes: Vec<AlignToWpChange>,
    /// Overlay change requests produced by the script, drained to the ECS by the
    /// script bridge.
    pub pending_overlay_changes: Vec<OverlayChange>,
    /// `AI_GoToWP` requests produced by the script, drained to the ECS by the
    /// script bridge.
    pub pending_goto_changes: Vec<GotoChange>,
    /// Item spawn requests queued by the world load, drained to the ECS by the
    /// script bridge.
    pub pending_spawn_items: Vec<SpawnItem>,
    /// Npc spawn requests queued by `wld_insertnpc`, drained to the ECS by the
    /// script bridge.
    pub pending_spawn_npcs: Vec<SpawnNpc>,
    /// Dialog lines output by the script (`AI_Output`), drained to the ECS by
    /// the script bridge.
    pub pending_dialog_lines: Vec<DialogLine>,
    /// Items the player's inventory gained through the script
    /// (`CreateInvItem(s)`, `Npc_GiveItem`), drained to the ECS by the script
    /// bridge for the game log.
    pub pending_inventory_gains: Vec<InventoryGain>,
    /// Instance id of the player (`PC_Hero`), set by the world spawn. Compared
    /// against by `Npc_IsPlayer` and the inventory-gain log.
    pub player_instance: Option<InstanceId>,
    /// Sub dialog choices added by the script (`Info_AddChoice`), drained to
    /// the ECS by the script bridge.
    pub pending_sub_dialog_choices: Vec<DialogRoot>,
    /// Guild to guild attitude matrix (`Wld_SetGuildAttitude`).
    pub guild_attitudes: HashMap<(i32, i32), i32>,
    /// World position of every waypoint/freepoint by lowercased name, filled at
    /// world load (`Npc_GetDistToWP`).
    pub way_points: HashMap<String, Vec3>,
    /// Generator backing `Hlp_Random`.
    pub rng: ScriptRandom,
}

impl ImportCallState {
    /// Sets the weapon mode of an npc, ignored when the id is not an npc.
    pub fn set_weapon_mode(&mut self, npc: i32, mode: WeaponMode) {
        let Some(npc) = self.symbol_table.instance(npc) else {
            return;
        };
        self.loaded_state
            .npc_instances
            .entry(npc)
            .or_default()
            .weapon_mode = Some(mode);
        self.pending_weapon_mode_changes
            .push(WeaponModeChange { instance_id: npc });
    }

    /// Queues an overlay apply or remove for an npc.
    pub fn set_overlay_request(&mut self, npc: i32, request: OverlayRequest) {
        let Some(npc) = self.symbol_table.instance(npc) else {
            return;
        };
        self.pending_overlay_changes.push(OverlayChange {
            instance_id: npc,
            request,
        });
    }

    /// Queues a `AI_GoToWP` request (waypoint name string symbol id) for an npc.
    pub fn set_goto_request(&mut self, npc: i32, waypoint: i32) {
        let Some(npc) = self.symbol_table.instance(npc) else {
            return;
        };
        self.pending_goto_changes.push(GotoChange {
            instance_id: npc,
            waypoint,
        });
    }

    /// Whether the npc's recorded (current/previous) routine state function
    /// equals `state`; `false` when either id is not valid.
    pub fn is_in_state(&self, npc: i32, state: i32, previous: bool) -> i32 {
        let Some(npc) = self.symbol_table.instance(npc) else {
            return 0;
        };
        let Some(state) = self.symbol_table.function(state) else {
            return 0;
        };
        let recorded = self
            .loaded_state
            .npc_instances
            .get(&npc)
            .and_then(|instance| {
                if previous {
                    instance.previous_routine_state
                } else {
                    instance.current_routine_state
                }
            });
        (recorded == Some(state)) as i32
    }

    /// Fetches the string content of a raw string table entry, `None` when the
    /// entry is empty (see `SymbolTable::string_content`). Resolved by a table
    /// loaded at startup: import handlers must not call back into the guest
    /// while a script function is running.
    pub(crate) fn get_string_non_0(&self, raw: i32) -> Option<String> {
        self.symbol_table.string_content(raw)
    }

    /// Fetches a static string literal with its original case
    pub(crate) fn get_show_string(&self, id: StringId) -> Option<ShowString> {
        let string = self.get_string_non_0(id.value())?;
        Some(ShowString::new(string))
    }

    /// Fetches a static string literal as a canonical (lowercased) lookup key,
    /// for names matched case-insensitively (waypoints, spots)
    pub(crate) fn get_canon_string(&self, id: StringId) -> Option<StringCanon> {
        let string = self.get_string_non_0(id.value())?;
        Some(StringCanon::new(string))
    }
}

/// Overlay change requested for an npc.
#[derive(Debug, Clone, Copy)]
pub enum OverlayRequest {
    /// Apply the overlay whose name is the given string symbol id.
    Apply(i32),
    /// Remove the current overlay.
    Remove,
}

/// Weapon mode of an npc, drives the idle animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponMode {
    /// No weapon drawn (fists or sheathed)
    Fists,
    /// Melee weapon drawn
    Melee,
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

/// Mersenne Twister generator backing `Hlp_Random`, fixed seed for
/// deterministic replays.
#[derive(Debug)]
pub struct ScriptRandom {
    rng: Mt,
}

impl Default for ScriptRandom {
    fn default() -> Self {
        Self {
            rng: Mt::new(0x9E37_79B9),
        }
    }
}

impl ScriptRandom {
    /// Advances the generator and returns the next raw value.
    pub fn next(&mut self) -> u32 {
        self.rng.next_u32()
    }
}
