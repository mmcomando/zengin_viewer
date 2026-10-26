mod changes;
mod instances;
mod weapons;

pub(crate) use changes::{
    AlignToWpChange, DialogClose, DialogLine, EquipItemChange, GotoChange, HealthChange,
    InventoryGain, NpcLookAtChange, OverlayChange, RoutineEntryChange, RoutineExchange,
    ScreenMessage, SpawnItem, SpawnNpc, WeaponModeChange,
};
pub(crate) use instances::{
    DialogChoice, DialogRoot, ItemInstanceState, NpcInstanceState, RoutineEntry,
};
pub(crate) use weapons::{OverlayRequest, WeaponKind, WeaponMode};

use std::collections::HashMap;
use std::sync::Arc;

use bevy::math::Vec3;
use rand_mt::Mt;

use super::bindings::MmWorld;
use super::bindings::exports::mmcomando::zengine_scripts::manage::SymbolKind;
use super::journal::{JournalChange, JournalManager};
use super::string_value::{ShowString, StringCanon};
use super::symbol_id::{FunctionId, InstanceId, StringId, SymbolId};
/// A mob state assignment scheduled by `Wld_SetMobRoutine`: the mob with
/// `mob_name` should be switched to `state` when the game time reaches
/// `hour`:`minute`. Stored for later consumers, mobs are not simulated yet.
#[allow(dead_code)]
#[derive(Debug)]
pub struct MobRoutineEntry {
    pub hour: i32,
    pub minute: i32,
    pub mob_name: String,
    pub state: i32,
}

#[derive(Debug, Default)]
pub struct LoadedState {
    /// State of every loaded item instance.
    pub items_instances: HashMap<InstanceId, ItemInstanceState>,
    /// State of every loaded npc instance.
    pub npc_instances: HashMap<InstanceId, NpcInstanceState>,
    /// Time-of-day mob states registered by `wld_setmobroutine`.
    pub mob_routines: Vec<MobRoutineEntry>,
    /// Sub dialog choices keyed by the owning `C_INFO` instance.
    pub dialog_choices: HashMap<InstanceId, Vec<DialogChoice>>,
}

/// Kind of every loaded symbol, keyed by symbol id. Used to verify typed
/// symbol ids ([`InstanceId`], [`FunctionId`], [`StringId`]) at conversion
/// points instead of trusting raw ids. Also holds the contents of static
/// string literals, loaded once at startup so import handlers never need to
/// call back into the guest to resolve string content, and the mirror of the
/// guest `DYN_STRINGS` ring for dynamic string ids.
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
    /// Raw id of the first `DYN_STRINGS` ring slot, dynamic string ids returned
    /// by the string-returning externals start here (the guest's `strings_num`).
    dynamic_strings_base: i32,
    /// Mirror of the guest `DYN_STRINGS` ring: raw content the host returned
    /// for each dynamic string slot, in call order which matches the guest ring
    /// cursor. Slots are overwritten by later dynamic strings, so content
    /// should be resolved through `string_content` right after the dynamic
    /// string was created.
    dynamic_strings: Vec<Vec<u8>>,
}

/// Number of `DYN_STRINGS` ring slots, must match the guest (`base_module.wat`)
const DYN_STRINGS_SLOTS_NUM: usize = 64;

impl SymbolTable {
    pub fn new(kinds: HashMap<i32, SymbolKind>, symbols_count: i32, strings: Vec<String>) -> Self {
        Self {
            kinds,
            symbols_count,
            dynamic_strings_base: strings.len() as i32,
            strings,
            dynamic_strings: Vec::new(),
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

    /// Name of a symbol by its raw id (ids below `symbols_count`), `None`
    /// when the id is out of range. Reads the preloaded string table, so it is
    /// safe to call from import handlers (no guest re-entry).
    pub fn symbol_name(&self, raw: i32) -> Option<String> {
        if raw < 0 || raw >= self.symbols_count {
            return None;
        }
        self.string_content(raw)
    }

    /// String literals are not symbols, their ids are indices into the strings
    /// table placed after all symbol ids. The id may also point at a
    /// `DYN_STRINGS` ring slot (a result of `concatstrings` and friends), whose
    /// content is overwritten by later dynamic strings — resolving content
    /// through `string_content` right away is always correct, caching the raw
    /// id for later is only safe for genuinely static strings.
    pub fn string(&self, raw: i32) -> Option<StringId> {
        if raw < self.symbols_count {
            return None;
        }
        StringId::new(raw)
    }

    /// Records the content a string-returning external returned, keeping the
    /// mirror ring in sync with the guest ring cursor. Must be called exactly
    /// once per string-returning external invocation, in call order.
    pub fn record_dynamic_string(&mut self, content: Vec<u8>) {
        if self.dynamic_strings.len() < DYN_STRINGS_SLOTS_NUM {
            self.dynamic_strings.push(content);
        } else {
            let slot = self.dynamic_strings.len() % DYN_STRINGS_SLOTS_NUM;
            self.dynamic_strings[slot] = content;
        }
    }

    /// Raw bytes of a string id of either kind: static table entry or
    /// `DYN_STRINGS` ring slot
    pub fn string_bytes(&self, raw: i32) -> Option<Vec<u8>> {
        if raw >= self.dynamic_strings_base {
            let slot = (raw - self.dynamic_strings_base) as usize;
            return self.dynamic_strings.get(slot).cloned();
        }
        self.strings
            .get(raw.max(0) as usize)
            .map(|el| el.as_bytes().to_vec())
    }

    /// Content of the string table entry `raw` (symbol name, static string
    /// literal or dynamic ring slot), `None` when the id is out of range. Empty
    /// strings are valid values: scripts pass `""` on purpose (e.g. no head
    /// model for animals), so callers decide whether an empty content means
    /// "absent". Dynamic content reflects the current ring slot value, so it
    /// should be resolved right after the dynamic string was created.
    pub fn string_content(&self, raw: i32) -> Option<String> {
        let bytes = self.string_bytes(raw)?;
        Some(String::from_utf8_lossy(&bytes).into_owned())
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
    /// Changes and requests produced by the script and its host functions,
    /// drained to the ECS by the script bridge, which turns them into messages
    /// for the `apply_*` systems.
    pub pending: PendingChanges,
    /// Instance id of the player (`PC_Hero`), set by the world spawn. Compared
    /// against by `Npc_IsPlayer` and the inventory-gain log.
    pub player_instance: Option<InstanceId>,
    /// Journal (quest log) topics, filled by the `Log_*` host functions.
    pub journal: JournalManager,
    /// Guild to guild attitude matrix (`Wld_SetGuildAttitude`).
    pub guild_attitudes: HashMap<(i32, i32), i32>,
    /// World position of every waypoint/freepoint by lowercased name, filled at
    /// world load (`Npc_GetDistToWP`).
    pub way_points: HashMap<String, Vec3>,
    /// Generator backing `Hlp_Random`.
    pub rng: ScriptRandom,
}

/// Changes and requests queued by the script vm, see [`ImportCallState::pending`].
#[derive(Default)]
pub struct PendingChanges {
    /// Routine entry changes produced by the script (`TA_Min` and friends).
    pub routine_changes: Vec<RoutineEntryChange>,
    /// Look-at target changes produced by the script (`AI_LookAt*`).
    pub look_at_changes: Vec<NpcLookAtChange>,
    /// Weapon mode changes produced by the script (`Npc_SetToFightMode`).
    pub weapon_mode_changes: Vec<WeaponModeChange>,
    /// Equipped item changes produced by the script (`EquipItem`).
    pub equip_item_changes: Vec<EquipItemChange>,
    /// Hitpoints changes produced by the host (`Npc_ChangeAttribute`).
    pub health_changes: Vec<HealthChange>,
    /// `AI_AlignToWP` requests produced by the script.
    pub align_to_wp_changes: Vec<AlignToWpChange>,
    /// Overlay change requests produced by the script.
    pub overlay_changes: Vec<OverlayChange>,
    /// `AI_GoToWP` requests produced by the script.
    pub goto_changes: Vec<GotoChange>,
    /// `Npc_ExchangeRoutine` requests produced by the script.
    pub routine_exchanges: Vec<RoutineExchange>,
    /// Item spawn requests queued by the world load and `wld_insertitem`.
    pub spawn_items: Vec<SpawnItem>,
    /// Npc spawn requests queued by `wld_insertnpc`.
    pub spawn_npcs: Vec<SpawnNpc>,
    /// Dialog lines output by the script (`AI_Output`).
    pub dialog_lines: Vec<DialogLine>,
    /// Items the player's inventory gained through the script
    /// (`CreateInvItem(s)`, `Npc_GiveItem`).
    pub inventory_gains: Vec<InventoryGain>,
    /// On-screen messages requested by `PrintScreen`/`AI_PrintScreen`.
    pub screen_messages: Vec<ScreenMessage>,
    /// Journal changes produced by the script (`Log_*` host functions).
    pub journal_changes: Vec<JournalChange>,
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
        self.pending
            .weapon_mode_changes
            .push(WeaponModeChange { instance_id: npc });
    }

    /// Queues an overlay apply or remove for an npc.
    pub fn set_overlay_request(&mut self, npc: i32, request: OverlayRequest) {
        let Some(npc) = self.symbol_table.instance(npc) else {
            return;
        };
        self.pending.overlay_changes.push(OverlayChange {
            instance_id: npc,
            request,
        });
    }

    /// Queues a `AI_GoToWP` request (waypoint name string symbol id) for an npc.
    pub fn set_goto_request(&mut self, npc: i32, waypoint: i32) {
        let Some(npc) = self.symbol_table.instance(npc) else {
            return;
        };
        self.pending.goto_changes.push(GotoChange {
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
        i32::from(recorded == Some(state))
    }

    /// Resolves both ids as npc/item instance symbols, `None` when either is
    /// not valid.
    pub(crate) fn npc_item_pair(&self, npc: i32, item: i32) -> Option<(InstanceId, InstanceId)> {
        let npc = self.symbol_table.instance(npc)?;
        let item = self.symbol_table.instance(item)?;
        Some((npc, item))
    }

    /// Fetches the string content of a raw string table entry, `None` only
    /// when the id is out of range (empty strings are valid values, see
    /// `SymbolTable::string_content`). Static entries are resolved by a table
    /// loaded at startup, dynamic ids resolve from the `DYN_STRINGS` mirror:
    /// import handlers must not call back into the guest while a script
    /// function is running.
    pub(crate) fn get_string_non_0(&self, raw: i32) -> Option<String> {
        self.symbol_table.string_content(raw)
    }

    /// Raw bytes of a string id of either kind (static table entry or dynamic
    /// ring slot), empty when the id is invalid
    pub(crate) fn get_string_bytes(&self, raw: i32) -> Vec<u8> {
        self.symbol_table.string_bytes(raw).unwrap_or_default()
    }

    /// Records the content a string-returning external returned into the
    /// `DYN_STRINGS` mirror ring, see `SymbolTable::record_dynamic_string`
    pub(crate) fn record_dynamic_string(&mut self, content: Vec<u8>) {
        self.symbol_table.record_dynamic_string(content);
    }

    /// Lowercased name of the way point nearest to the npc position, `None`
    /// when the npc has no known position or the way net has no waypoints
    pub(crate) fn nearest_way_point(&self, npc: i32) -> Option<String> {
        let npc = self.symbol_table.instance(npc)?;
        let position = self
            .loaded_state
            .npc_instances
            .get(&npc)
            .and_then(|npc| npc.position)?;
        let (name, _) = self.way_points.iter().min_by(|left, right| {
            let (da, db) = (
                (left.1 - position).length_squared(),
                (right.1 - position).length_squared(),
            );
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })?;
        Some(name.clone())
    }

    /// Queues an on-screen message for the ECS bridge, empty messages are
    /// dropped (the engine ignores them too)
    pub(crate) fn push_screen_message(&mut self, msg: i32, x: i32, y: i32, time: i32) {
        let Some(msg_id) = self.symbol_table.string(msg) else {
            crate::warn_unimplemented!("printscreen with non-string text({msg})");
            return;
        };
        let Some(text) = self.get_string_non_0(msg_id.value()) else {
            return;
        };
        self.pending.screen_messages.push(ScreenMessage {
            text,
            x,
            y,
            time_sec: time,
        });
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
