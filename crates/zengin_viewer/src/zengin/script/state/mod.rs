#![allow(clippy::multiple_inherent_impl)]

mod body_state;
mod changes;
mod instances;
mod weapons;

pub(crate) use body_state::{
    BS_ITEMINTERACT, BS_LIE, BS_SIT, BS_STAND, MobSchemeLists, body_state_for_scheme,
    current_body_state, has_body_flag,
};
pub(crate) use changes::{
    AlignToWpChange, AnimationChange, AttackChange, ChapterIntro, ClearAiQueueChange, DialogClose,
    DialogLine, GotoChange, GotoFpChange, GotoNpcChange, InventoryGain, ItemChanged,
    ItemEntitiesChanged, NpcChanged, NpcEntitiesChanged, OverlayChange, PassivePerception,
    ProcessInfosChange, RoutineExchange, ScreenMessage, ScriptSound, SetGameTime, SpawnItem,
    SpawnNpc, StandUpChange, StartStateChange, SvmLine, TurnToNpcChange, UseItemStateChange,
    UseMobChange, WaitChange,
};
pub(crate) use instances::{
    DialogChoice, DialogRoot, ItemRuntime, NpcCheckInfo, NpcRuntime, RoutineEntry,
    RoutineFunctions, TALENT_1H,
};
pub(crate) use weapons::{ITEM_MAINFLAG_ARMOR, OverlayRequest, WeaponKind, WeaponMode};

use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::{Arc, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};

use bevy::log::warn;
use bevy::math::Vec3;
use daedalus_typed::TypedRegistry;
use rand_mt::Mt;
use serde::{Deserialize, Serialize};

use super::bindings::exports::mmcomando::zengine_scripts::manage::SymbolKind;
use super::journal::{JournalChange, JournalManager};
use super::string_value::{ShowString, StringCanon};
use super::symbol_id::{FunctionId, InstanceId, StringId, SymbolId};
use crate::external_messages::ExternalMessageSender;
use crate::zengin::common::{
    DEFAULT_SIGHT_RANGE_METERS, FREEPOINT_SEARCH_RADIUS, MAX_AI_USE_DISTANCE, MOB_SEARCH_RADIUS,
    horizontal_offset, to_world_units,
};
use bevy::ecs::resource::Resource;

/// `PERC_ASSESSPLAYER` script constant, the perception whose callback
/// (`b_assessplayer`) starts npc initiated dialogs.
pub const PERC_ASSESSPLAYER: i32 = 1;

/// `PERC_ASSESSENEMY` script constant, the perception that starts a fight.
pub const PERC_ASSESSENEMY: i32 = 2;

/// `PERC_ASSESSDAMAGE` script constant, the perception delivered to the victim
/// of a damage event.
pub const PERC_ASSESSDAMAGE: i32 = 8;

/// Cosine of the `Npc_CanSeeNpc` ±100° view range.
const VIEW_CONE_COS: f32 = -0.173_648_18;

/// A mob state assignment scheduled by `Wld_SetMobRoutine`: the mob with
/// `mob_name` should be switched to `state` when the game time reaches
/// `hour`:`minute`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MobRoutineEntry {
    pub hour: i32,
    pub minute: i32,
    pub mob_name: String,
    pub state: i32,
}

#[derive(Debug, Default)]
pub struct LoadedState {
    /// State of every loaded item instance.
    pub items_instances: HashMap<InstanceId, Arc<RwLock<ItemRuntime>>>,
    /// State of every loaded npc instance.
    pub npc_instances: HashMap<InstanceId, Arc<RwLock<NpcRuntime>>>,
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
    /// Total number of dynamic strings recorded so far, mirrors the guest's
    /// unbounded `dyn_strings_next_slot` cursor. `dynamic_strings` only stores
    /// the ring slots, so its length stops growing at `DYN_STRINGS_SLOTS_NUM`
    /// and cannot be used to derive the current slot.
    dynamic_strings_recorded: u64,
    /// Symbol ids by lowercased name, for host functions which receive a
    /// symbol name as a script string argument.
    names: HashMap<String, i32>,
    /// Runtime instance copies created for duplicated spawns, copy id -> source
    /// symbol id. Copy ids live above the whole string id range.
    copies: HashMap<i32, i32>,
}

/// Number of `DYN_STRINGS` ring slots, must match the guest (`base_module.wat`)
const DYN_STRINGS_SLOTS_NUM: usize = 64;

impl SymbolTable {
    pub fn new(kinds: HashMap<i32, SymbolKind>, symbols_count: i32, strings: Vec<String>) -> Self {
        let names = strings
            .iter()
            .take(usize::try_from(symbols_count).unwrap_or_default())
            .enumerate()
            .map(|(index, name)| (name.to_lowercase(), index as i32))
            .collect();
        Self {
            kinds,
            symbols_count,
            dynamic_strings_base: strings.len() as i32,
            strings,
            dynamic_strings: Vec::new(),
            dynamic_strings_recorded: 0,
            names,
            copies: HashMap::new(),
        }
    }

    /// First instance copy id. Copies must not collide with symbol ids, static
    /// string literals or the dynamic string ring.
    pub fn copy_instance_base(&self) -> i32 {
        self.dynamic_strings_base + DYN_STRINGS_SLOTS_NUM as i32 + 1
    }

    /// Marks `copy` as a runtime copy of the instance symbol `source`.
    pub fn register_instance_copy(&mut self, copy: i32, source: i32) {
        self.copies.insert(copy, source);
    }

    /// Drops every runtime instance copy registration.
    pub fn clear_copies(&mut self) {
        self.copies.clear();
    }

    fn verified(&self, raw: i32, expected: SymbolKind) -> Option<SymbolId> {
        match self.kinds.get(&raw) {
            Some(kind) if *kind == expected => SymbolId::new(raw),
            _ => None,
        }
    }

    pub fn instance(&self, raw: i32) -> Option<InstanceId> {
        if self.copies.contains_key(&raw) {
            return InstanceId::new(raw);
        }
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
        if let Some(source) = self.copies.get(&raw) {
            return self.symbol_name(*source);
        }
        if raw < 0 || raw >= self.symbols_count {
            return None;
        }
        self.string_content(raw)
    }

    /// Raw symbol id of a lowercased symbol name, `None` when the name is
    /// unknown. Safe to call from import handlers (no guest re-entry).
    pub fn symbol_id_by_name(&self, name: &str) -> Option<i32> {
        self.names.get(name).copied()
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

    /// Whether a string id points at the dynamic `DYN_STRINGS` ring instead of
    /// the static table. Dynamic content is overwritten by later calls, so such
    /// ids must not be persisted.
    pub fn is_dynamic_string_id(&self, raw: i32) -> bool {
        raw >= self.dynamic_strings_base
    }

    /// Records the content a string-returning external returned, keeping the
    /// mirror ring in sync with the guest ring cursor. Must be called exactly
    /// once per string-returning external invocation, in call order.
    pub fn record_dynamic_string(&mut self, content: Vec<u8>) {
        let slot = (self.dynamic_strings_recorded % DYN_STRINGS_SLOTS_NUM as u64) as usize;
        match self.dynamic_strings.get_mut(slot) {
            Some(entry) => *entry = content,
            None => self.dynamic_strings.push(content),
        }
        self.dynamic_strings_recorded += 1;
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

/// Way-net freepoint mirrored for the freepoint host functions.
#[derive(Debug, Clone)]
pub struct ScriptFreePoint {
    /// Lowercased name.
    pub name: String,
    /// World position, Bevy units (meters).
    pub position: Vec3,
}

/// Interactive mob mirrored from the world for the mob host functions.
#[derive(Debug, Clone)]
pub struct ScriptMob {
    /// World position, Bevy units (meters).
    pub position: Vec3,
    /// Every interaction point is taken.
    pub occupied: bool,
    /// State applied by `Wld_SetMobRoutine`.
    pub state: i32,
}

#[derive(Default)]
pub struct ImportCallState {
    /// Typed models of the G2 script classes which have one.
    pub typed: TypedRegistry,
    /// Symbol ids of the well known instance variables, resolved when the
    /// symbol table is loaded.
    pub well_known_symbols: WellKnownSymbols,
    pub loaded_state: LoadedState,
    pub close_dialog: bool,
    /// A dialog session is open (`InfoManager_HasFinished`).
    pub dialog_active: bool,
    /// Kind of every symbol, used to verify typed symbol ids
    pub symbol_table: SymbolTable,
    pub messages: ScriptMessages,
    pub pending: PendingChanges,
    /// Instance id of the player (`PC_Hero`), set by the world spawn.
    pub player_instance: Option<InstanceId>,
    /// Current game day reported by `Wld_GetDay`, 0 on the first day.
    pub game_day: u32,
    /// Current in-game minute of the day, `Wld_IsTime` range checks.
    pub game_minutes_of_day: u32,
    /// Running clock in seconds, used by time limited script state such as
    /// `Npc_SetRefuseTalk`.
    pub world_seconds: f64,
    /// Journal (quest log) topics, filled by the `Log_*` host functions.
    pub journal: JournalManager,
    /// Guild to guild attitude matrix (`Wld_SetGuildAttitude`).
    pub guild_attitudes: HashMap<(i32, i32), i32>,
    /// Perception ranges in engine units (`Perc_SetRange`), set once by the
    /// script `initperceptions` at world load.
    pub perc_ranges: HashMap<i32, i32>,
    /// Spawned object ids per instance symbol. `Wld_InsertNpc` may spawn the
    /// same instance several times, later spawns become runtime copies with
    /// their own typed record and runtime state.
    pub spawned_npcs: HashMap<InstanceId, Vec<InstanceId>>,
    /// Next runtime copy instance id, `None` until the first copy is allocated.
    pub next_copy_instance_id: Option<i32>,
    /// Cached `Npc_CheckInfo` answers per npc. Refreshed before script calls
    /// which consult it.
    pub check_info: HashMap<InstanceId, NpcCheckInfo>,
    /// World position of every waypoint/freepoint by lowercased name, filled at
    /// world load (`Npc_GetDistToWP`).
    pub way_points: HashMap<String, Vec3>,
    /// Interactive mobs by lowercased scheme, refreshed by the world side when
    /// the mob set or their occupancy changes.
    pub mobs: HashMap<String, Vec<ScriptMob>>,
    /// Container contents by lowercased vob name (`Mob_HasItems`), refreshed by
    /// the world side when the chests spawn or their loot changes. Unnamed
    /// chests are absent, the engine cannot look them up either.
    pub containers: HashMap<String, Vec<(InstanceId, i32)>>,
    /// Way-net freepoints, filled at world load.
    pub freepoints: Vec<ScriptFreePoint>,
    /// Npc currently heading to or standing on a freepoint, by point name.
    pub freepoint_users: HashMap<String, InstanceId>,
    /// Freepoint the npc is currently using, by npc instance.
    pub npc_freepoint: HashMap<InstanceId, String>,
    /// Generator backing `Hlp_Random`.
    pub rng: ScriptRandom,
}

/// Symbol ids of the well known daedalus instance variables, resolved once when
/// the symbol table is loaded. `None` when the script does not declare them.
#[derive(Debug, Default, Clone, Copy)]
#[expect(
    clippy::struct_field_names,
    reason = "Fields name the well known daedalus instance variables"
)]
pub struct WellKnownSymbols {
    pub self_symbol: Option<i32>,
    pub other_symbol: Option<i32>,
    pub victim_symbol: Option<i32>,
    pub item_symbol: Option<i32>,
    pub hero_symbol: Option<i32>,
}

/// Changes and requests queued by the script vm, see [`ImportCallState::pending`].
#[derive(Default)]
pub struct PendingChanges {
    /// `Npc_ExchangeRoutine` requests produced by the script.
    pub routine_exchanges: Vec<RoutineExchange>,
    /// Chapter splash requested by `IntroduceChapter`, at most one pending at a
    /// time (a later request replaces an undrained one).
    pub chapter_intro: Option<ChapterIntro>,
    /// Npc instances whose data was edited since the last drain, deduplicated
    /// so one frame produces at most one message per instance.
    pub npc_changed: BTreeSet<InstanceId>,
    /// Item instances whose data was edited since the last drain.
    pub item_changed: BTreeSet<InstanceId>,
}

/// Queues of the changes the script produces for the ECS. They live outside
/// [`PendingChanges`] because a world restart resets that wholesale while the
/// sends must keep working.
#[derive(Clone, Default, Resource)]
pub struct ScriptMessages {
    /// `AI_AlignToWP` requests produced by the script.
    pub align_to_wp: ExternalMessageSender<AlignToWpChange>,
    /// Overlay change requests produced by the script.
    pub overlay: ExternalMessageSender<OverlayChange>,
    /// `AI_GoToWP` requests produced by the script.
    pub goto: ExternalMessageSender<GotoChange>,
    /// `AI_GotoFP`/`AI_GotoNextFP` requests produced by the script host.
    pub goto_fp: ExternalMessageSender<GotoFpChange>,
    /// `AI_GotoNpc` requests produced by the script.
    pub goto_npc: ExternalMessageSender<GotoNpcChange>,
    /// `AI_Attack` requests produced by the script.
    pub attack: ExternalMessageSender<AttackChange>,
    /// `AI_TurnToNPC` requests produced by the script.
    pub turn_to_npc: ExternalMessageSender<TurnToNpcChange>,
    /// `AI_ProcessInfos` requests produced by the script.
    pub process_infos: ExternalMessageSender<ProcessInfosChange>,
    /// `AI_PlayAni`/`AI_PlayAniBS` requests produced by the script.
    pub animation: ExternalMessageSender<AnimationChange>,
    /// `AI_StartState` requests produced by the script.
    pub start_state: ExternalMessageSender<StartStateChange>,
    /// `AI_UseMob` requests produced by the script.
    pub use_mob: ExternalMessageSender<UseMobChange>,
    /// `AI_UseItemToState` requests produced by the script.
    pub use_item_state: ExternalMessageSender<UseItemStateChange>,
    /// `AI_WaitMS` requests produced by the script.
    pub wait: ExternalMessageSender<WaitChange>,
    /// `AI_StandUp`/`AI_StandUpQuick` requests produced by the script.
    pub stand_up: ExternalMessageSender<StandUpChange>,
    /// `Npc_ClearAIQueue` requests produced by the script.
    pub clear_ai_queue: ExternalMessageSender<ClearAiQueueChange>,
    /// Item spawn requests queued by the world load and `wld_insertitem`.
    pub spawn_items: ExternalMessageSender<SpawnItem>,
    /// Npc spawn requests queued by `wld_insertnpc`.
    pub spawn_npcs: ExternalMessageSender<SpawnNpc>,
    /// Dialog lines output by the script (`AI_Output`).
    pub dialog_lines: ExternalMessageSender<DialogLine>,
    /// Items the player's inventory gained through the script
    /// (`CreateInvItem(s)`, `Npc_GiveItem`).
    pub inventory_gains: ExternalMessageSender<InventoryGain>,
    /// On-screen messages requested by `PrintScreen`/`AI_PrintScreen`.
    pub screen_messages: ExternalMessageSender<ScreenMessage>,
    /// Sounds requested by `Snd_Play`/`Snd_Play3D` and the `AI_Snd_Play*`
    /// variants.
    pub sounds: ExternalMessageSender<ScriptSound>,
    /// Speech lines requested by `AI_OutputSVM`/`AI_OutputSVM_Overlay`.
    pub svm: ExternalMessageSender<SvmLine>,
    /// `Npc_SendPassivePerc` broadcasts produced by the script.
    pub passive_perception: ExternalMessageSender<PassivePerception>,
    /// Journal changes produced by the script (`Log_*` host functions).
    pub journal_changes: ExternalMessageSender<JournalChange>,
    /// `Wld_SetTime` requests produced by the script.
    pub game_time: ExternalMessageSender<SetGameTime>,
}

impl ScriptMessages {
    /// Drops every message the bridge has not forwarded to the ECS yet.
    pub fn clear(&self) {
        self.align_to_wp.clear();
        self.overlay.clear();
        self.goto.clear();
        self.goto_fp.clear();
        self.goto_npc.clear();
        self.attack.clear();
        self.turn_to_npc.clear();
        self.process_infos.clear();
        self.animation.clear();
        self.start_state.clear();
        self.use_mob.clear();
        self.use_item_state.clear();
        self.wait.clear();
        self.stand_up.clear();
        self.clear_ai_queue.clear();
        self.spawn_items.clear();
        self.spawn_npcs.clear();
        self.dialog_lines.clear();
        self.inventory_gains.clear();
        self.screen_messages.clear();
        self.sounds.clear();
        self.svm.clear();
        self.passive_perception.clear();
        self.journal_changes.clear();
        self.game_time.clear();
    }
}

impl ImportCallState {
    /// Mutable runtime state of an npc, records the instance as changed.
    pub fn npc_state_mut(&mut self, instance: InstanceId) -> RwLockWriteGuard<'_, NpcRuntime> {
        self.pending.npc_changed.insert(instance);
        let state = self.loaded_state.npc_instances.entry(instance).or_default();
        state.write().unwrap_or_else(PoisonError::into_inner)
    }

    /// Runtime state of an npc, `None` when the instance has none.
    pub fn npc_state(&self, instance: InstanceId) -> Option<RwLockReadGuard<'_, NpcRuntime>> {
        Some(
            self.loaded_state
                .npc_instances
                .get(&instance)?
                .read()
                .unwrap_or_else(PoisonError::into_inner),
        )
    }

    /// Mutable runtime state of an item, records the instance as changed.
    pub fn item_state_mut(&mut self, instance: InstanceId) -> RwLockWriteGuard<'_, ItemRuntime> {
        self.pending.item_changed.insert(instance);
        let state = self
            .loaded_state
            .items_instances
            .entry(instance)
            .or_default();
        state.write().unwrap_or_else(PoisonError::into_inner)
    }

    /// Runtime state of an item, `None` when the instance has none.
    #[allow(dead_code)] // no item consumers yet
    pub fn item_state(&self, instance: InstanceId) -> Option<RwLockReadGuard<'_, ItemRuntime>> {
        Some(
            self.loaded_state
                .items_instances
                .get(&instance)?
                .read()
                .unwrap_or_else(PoisonError::into_inner),
        )
    }

    /// Nearest free mob with `scheme` within [`MOB_SEARCH_RADIUS`] of the npc,
    /// the way `Wld_IsMobAvailable`/`Wld_GetMobState` see the world.
    pub fn available_mob(&self, npc: InstanceId, scheme: &str) -> Option<&ScriptMob> {
        let position = {
            let npc = self.npc_state(npc)?;
            npc.live.position?
        };
        nearest_free_mob(self.mobs.get(scheme)?, position)
    }

    /// Count of `item` in the named container, `0` when the container or the
    /// item is unknown. `name` has to be canonical (lowercased).
    pub fn container_item_count(&self, name: &str, item: InstanceId) -> i32 {
        self.containers.get(name).map_or(0, |items| {
            items
                .iter()
                .find(|(entry, _)| *entry == item)
                .map_or(0, |(_, count)| *count)
        })
    }

    /// Nearest live npc other than `npc` within `radius` (world units) matching
    /// the filters, `None` when the filters match nothing. A `None` filter
    /// matches every value; ties are resolved by the lower instance id.
    #[allow(clippy::float_cmp)]
    pub fn nearest_npc(
        &self,
        npc: InstanceId,
        radius: f32,
        instance: Option<InstanceId>,
        state: Option<FunctionId>,
        guild: Option<i32>,
        exclude_player: bool,
    ) -> Option<InstanceId> {
        let position = self.npc_state(npc)?.live.position?;
        if radius <= 0.0 {
            return None;
        }
        let radius_squared = radius * radius;
        let player = self.player_instance;
        let mut best: Option<(InstanceId, f32)> = None;
        for (&candidate, runtime) in &self.loaded_state.npc_instances {
            if candidate == npc {
                continue;
            }
            if instance.is_some_and(|filter| filter != candidate) {
                continue;
            }
            if exclude_player && Some(candidate) == player {
                continue;
            }
            let runtime = runtime.read().unwrap_or_else(PoisonError::into_inner);
            if runtime.live.dead {
                continue;
            }
            let Some(candidate_position) = runtime.live.position else {
                continue;
            };
            if state.is_some_and(|filter| runtime.routine.current_state != Some(filter)) {
                continue;
            }
            drop(runtime);
            if let Some(guild) = guild {
                let candidate_guild = self
                    .typed
                    .npc(candidate.value() as u32)
                    .map_or(0, |npc| npc.guild);
                if candidate_guild != guild {
                    continue;
                }
            }
            let distance = (candidate_position - position).length_squared();
            if distance > radius_squared {
                continue;
            }
            let better = match best {
                None => true,
                Some((best_id, best_distance)) => {
                    distance < best_distance || (distance == best_distance && candidate < best_id)
                }
            };
            if better {
                best = Some((candidate, distance));
            }
        }
        best.map(|(instance, _)| instance)
    }

    /// Canonical (lowercased) scheme of a mob name string id.
    pub fn mob_scheme(&self, raw: i32) -> Option<StringCanon> {
        let id = self.symbol_table.string(raw)?;
        self.get_canon_string(id)
    }

    /// Canonical (lowercased) content of a string symbol id argument.
    pub fn canon_string_arg(&self, raw: i32) -> Option<StringCanon> {
        let id = self.symbol_table.string(raw)?;
        self.get_canon_string(id)
    }

    /// Nearest freepoint whose name contains `query` within
    /// [`FREEPOINT_SEARCH_RADIUS`] of the npc, skipping points held by another
    /// npc. `next` also excludes the point the npc is standing on.
    pub fn available_freepoint(
        &self,
        npc: InstanceId,
        query: &str,
        next: bool,
    ) -> Option<&ScriptFreePoint> {
        let position = {
            let npc = self.npc_state(npc)?;
            npc.live.position?
        };
        let current = self.npc_freepoint.get(&npc).map(String::as_str);
        let radius_squared = FREEPOINT_SEARCH_RADIUS * FREEPOINT_SEARCH_RADIUS;
        self.freepoints
            .iter()
            .filter(|point| point.name.contains(query))
            .filter(|point| !next || Some(point.name.as_str()) != current)
            .filter(|point| {
                self.freepoint_users
                    .get(&point.name)
                    .is_none_or(|user| *user == npc)
            })
            .map(|point| (point, point.position.distance_squared(position)))
            .filter(|(_, distance)| *distance <= radius_squared)
            .min_by(|(_, left), (_, right)| left.total_cmp(right))
            .map(|(point, _)| point)
    }

    /// Marks a freepoint as used by the npc, releasing its previous one.
    pub fn assign_freepoint(&mut self, npc: InstanceId, point_name: &str) {
        self.release_freepoint(npc);
        self.freepoint_users.insert(point_name.to_string(), npc);
        self.npc_freepoint.insert(npc, point_name.to_string());
    }

    /// Frees the freepoint held by the npc.
    pub fn release_freepoint(&mut self, npc: InstanceId) {
        let Some(point_name) = self.npc_freepoint.remove(&npc) else {
            return;
        };
        self.freepoint_users.remove(&point_name);
    }

    /// Resolves a freepoint for `AI_GotoFP`/`AI_GotoNextFP`, holds it for the
    /// npc and queues the walk.
    pub fn goto_freepoint(&mut self, npc: i32, waypoint: i32, next: bool) {
        let Some(npc_id) = self.symbol_table.instance(npc) else {
            return;
        };
        let Some(query) = self.canon_string_arg(waypoint) else {
            return;
        };
        let Some(point_name) = self
            .available_freepoint(npc_id, query.as_str(), next)
            .map(|point| point.name.clone())
        else {
            bevy::log::debug!("AI_GotoFP npc({npc}) has no free freepoint({query})");
            return;
        };
        self.assign_freepoint(npc_id, &point_name);
        bevy::log::debug!("npc({npc}) AI_GotoFP freepoint({point_name})");
        self.messages.goto_fp.send(GotoFpChange {
            instance_id: npc_id,
            point_name: StringCanon::new(point_name),
        });
    }

    /// Freepoint the npc is heading to or standing on.
    pub fn npc_freepoint(&self, npc: InstanceId) -> Option<&str> {
        self.npc_freepoint.get(&npc).map(String::as_str)
    }

    /// `Wld_IsFPAvailable` / `Wld_IsNextFPAvailable` as a script return value.
    pub fn freepoint_available(&self, npc: i32, fp_name: i32, next: bool) -> i32 {
        let Some(npc) = self.symbol_table.instance(npc) else {
            return 0;
        };
        let Some(query) = self.canon_string_arg(fp_name) else {
            return 0;
        };
        i32::from(
            self.available_freepoint(npc, query.as_str(), next)
                .is_some(),
        )
    }

    /// Whether the npc stands on a freepoint whose name contains `query`.
    pub fn npc_is_on_freepoint(&self, npc: InstanceId, query: &str) -> bool {
        let Some(point_name) = self.npc_freepoint(npc) else {
            return false;
        };
        if !point_name.contains(query) {
            return false;
        }
        let Some(point) = self
            .freepoints
            .iter()
            .find(|point| point.name == point_name)
        else {
            return false;
        };
        self.npc_state(npc)
            .and_then(|npc| npc.live.position)
            .is_some_and(|position| point.position.distance(position) <= MAX_AI_USE_DISTANCE)
    }

    /// Sets the weapon mode of an npc, ignored when the id is not an npc.
    pub fn set_weapon_mode(&mut self, npc: i32, mode: WeaponMode) {
        let Some(npc) = self.symbol_table.instance(npc) else {
            return;
        };
        self.npc_state_mut(npc).equipment.set_weapon_mode(mode);
    }

    /// Queues an overlay apply or remove for an npc.
    pub fn set_overlay_request(&mut self, npc: i32, request: OverlayRequest) {
        let Some(npc) = self.symbol_table.instance(npc) else {
            return;
        };
        self.messages.overlay.send(OverlayChange {
            instance_id: npc,
            request,
        });
    }

    /// Queues a `AI_GoToWP` request (waypoint name string symbol id) for an npc.
    pub fn set_goto_request(&mut self, npc: i32, waypoint: i32) {
        let Some(npc) = self.symbol_table.instance(npc) else {
            return;
        };
        self.messages.goto.send(GotoChange {
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
        let recorded = self.npc_state(npc).and_then(|instance| {
            if previous {
                instance.routine.previous_state
            } else {
                instance.routine.current_state
            }
        });
        i32::from(recorded == Some(state))
    }

    /// Instance bound to the `other` script global, `None` when it is unset or
    /// not a valid instance id.
    pub fn other_instance(&self) -> Option<InstanceId> {
        let symbol = self.well_known_symbols.other_symbol?;
        InstanceId::new(self.typed.read_symbol(symbol as u32, 0))
    }

    /// Binds an instance to the `other` script global.
    pub fn set_other(&mut self, instance: InstanceId) {
        let Some(symbol) = self.well_known_symbols.other_symbol else {
            return;
        };
        self.typed.write_symbol(symbol as u32, 0, instance.value());
    }

    /// Clears the `other` script global.
    pub fn clear_other(&mut self) {
        let Some(symbol) = self.well_known_symbols.other_symbol else {
            return;
        };
        self.typed.write_symbol(symbol as u32, 0, 0);
    }

    /// Perception range in engine units, `default` when the script did not set
    /// one (`Perc_SetRange` runs from `initperceptions` at world load).
    pub fn perc_range(&self, perception: i32, default: i32) -> i32 {
        self.perc_ranges
            .get(&perception)
            .copied()
            .unwrap_or(default)
    }

    /// Rebuilds the guild attitude matrix from a square table symbol
    /// (`GIL_ATTITUDES`): entry `i * side + j` is the attitude of guild `i`
    /// toward guild `j`.
    pub fn exchange_guild_attitudes(&mut self, symbol: u32) {
        let Some(values) = self
            .typed
            .variable(symbol)
            .map(|values| values.words().to_vec())
        else {
            warn!("guild attitude table symbol({symbol}) has no value slot");
            return;
        };
        let side = (values.len() as f64).sqrt() as usize;
        if side == 0 || side * side != values.len() {
            warn!(
                "guild attitude table symbol({symbol}) is not a square table({})",
                values.len()
            );
            return;
        }
        self.guild_attitudes.clear();
        for guild in 0..side {
            for other in 0..side {
                self.guild_attitudes
                    .insert((guild as i32, other as i32), values[guild * side + other]);
            }
        }
        bevy::log::info!("Guild attitude matrix {side}x{side} from symbol({symbol})");
    }

    /// Allocates the script object id for one `Wld_InsertNpc` call. The first
    /// spawn of an instance keeps the symbol id, later spawns get an independent
    /// runtime copy with its own typed `C_Npc` record and runtime state.
    pub fn allocate_npc_object(&mut self, source: InstanceId) -> InstanceId {
        let already_spawned = self
            .spawned_npcs
            .get(&source)
            .is_some_and(|spawned| !spawned.is_empty());
        if !already_spawned {
            self.spawned_npcs.entry(source).or_default().push(source);
            return source;
        }
        let next = self
            .next_copy_instance_id
            .unwrap_or_else(|| self.symbol_table.copy_instance_base());
        let Some(copy) = InstanceId::new(next) else {
            warn!(
                "Cannot allocate a copy of npc({}) at id({next}), spawning the shared instance",
                source.value()
            );
            return source;
        };
        self.next_copy_instance_id = Some(copy.value() + 1);
        let mut runtime = self.npc_state(source).map_or_default(|npc| npc.clone());
        // The copy has no entity yet, its sight must not inherit the source's.
        runtime.perception.seen = None;
        if !self
            .typed
            .copy_instance(source.value() as u32, copy.value() as u32)
        {
            warn!(
                "No typed C_Npc data for npc({}), spawning the shared instance",
                source.value()
            );
            return source;
        }
        self.symbol_table
            .register_instance_copy(copy.value(), source.value());
        *self.npc_state_mut(copy) = runtime;
        self.spawned_npcs.entry(source).or_default().push(copy);
        // bevy::log::info!(
        //     "Wld_InsertNpc npc({}) spawned again as copy({})",
        //     source.value(),
        //     copy.value()
        // );
        copy
    }

    /// Drops every duplicated-spawn registration, runtime instance copy and the
    /// copy runtime state.
    pub fn reset_npc_copies(&mut self) {
        let copies: Vec<InstanceId> = self
            .typed
            .copy_ids()
            .filter_map(|id| i32::try_from(id).ok())
            .filter_map(InstanceId::new)
            .collect();
        for copy in copies {
            self.loaded_state.npc_instances.remove(&copy);
        }
        self.spawned_npcs.clear();
        self.next_copy_instance_id = None;
        self.symbol_table.clear_copies();
        self.typed.clear_copies();
    }

    /// Attitude `npc` holds toward `other`: the non-player side's temporary or
    /// permanent attitude, else the guild attitude matrix.
    pub fn npc_attitude(&self, npc: InstanceId, other: InstanceId, temporary: bool) -> i32 {
        let player = self.player_instance;
        let speaker = match (Some(npc) == player, Some(other) == player) {
            (false, true) => Some(npc),
            (true, false) => Some(other),
            _ => None,
        };
        if let Some(speaker) = speaker {
            let attitudes = self
                .npc_state(speaker)
                .map(|speaker| (speaker.social.temp_attitude, speaker.social.attitude));
            if let Some((temp_attitude, attitude)) = attitudes {
                if temporary && let Some(attitude) = temp_attitude {
                    return attitude;
                }
                if let Some(attitude) = attitude {
                    return attitude;
                }
            }
        }

        let guild =
            |instance: InstanceId| self.typed.npc(instance.value() as u32).map(|npc| npc.guild);
        match (guild(npc), guild(other)) {
            (Some(guild_npc), Some(guild_other)) => {
                // Guilds outside the human guild table (monsters) have no row:
                // same guild is friendly, anything else hostile.
                let same_guild = guild_npc == guild_other;
                self.guild_attitudes
                    .get(&(guild_npc, guild_other))
                    .copied()
                    .unwrap_or(if same_guild {
                        super::host::ATT_FRIENDLY
                    } else {
                        super::host::ATT_HOSTILE
                    })
            }
            _ => super::host::ATT_NEUTRAL,
        }
    }

    /// Whether `npc` considers `other` hostile (`Npc::isEnemy`).
    pub fn is_enemy(&self, npc: InstanceId, other: InstanceId) -> bool {
        self.npc_attitude(npc, other, true) == super::host::ATT_HOSTILE
    }

    /// Nearest live npc `npc` is hostile to and can sense within its
    /// `PERC_ASSESSENEMY` range, the way `Npc::updateNearestEnemy` picks one.
    /// The cached sight set filters candidates when the world side built it;
    /// npcs without a set fall back to distance only.
    pub fn nearest_enemy(&self, npc: InstanceId) -> Option<InstanceId> {
        let (position, seen) = {
            let state = self.npc_state(npc)?;
            (state.live.position?, state.perception.seen.clone())
        };
        let senses_range = self
            .typed
            .npc(npc.value() as u32)
            .map_or(0, |npc| npc.senses_range);
        let range = self.perc_range(PERC_ASSESSENEMY, senses_range);
        if range <= 0 {
            return None;
        }
        let range_squared = to_world_units(range as f32).powi(2);
        let mut best: Option<(InstanceId, f32)> = None;
        for (&candidate, runtime) in &self.loaded_state.npc_instances {
            if candidate == npc {
                continue;
            }
            if seen.as_ref().is_some_and(|seen| !seen.contains(&candidate)) {
                continue;
            }
            let candidate_position = {
                let runtime = runtime.read().unwrap_or_else(PoisonError::into_inner);
                if runtime.live.dead {
                    continue;
                }
                let Some(candidate_position) = runtime.live.position else {
                    continue;
                };
                candidate_position
            };
            let distance = (candidate_position - position).length_squared();
            if distance > range_squared || !self.is_enemy(npc, candidate) {
                continue;
            }
            let better = match best {
                None => true,
                Some((best_id, best_distance)) => match distance.total_cmp(&best_distance) {
                    std::cmp::Ordering::Less => true,
                    std::cmp::Ordering::Equal => candidate < best_id,
                    std::cmp::Ordering::Greater => false,
                },
            };
            if better {
                best = Some((candidate, distance));
            }
        }
        best.map(|(instance, _)| instance)
    }

    /// Sight radius of `npc` in world units: the `senses_range` the engine uses
    /// for `Npc_CanSeeNpc`, the default when the script left it unset.
    pub fn sight_range(&self, npc: InstanceId) -> f32 {
        self.typed
            .npc(npc.value() as u32)
            .map(|npc| to_world_units(npc.senses_range as f32))
            .filter(|range| *range > 0.0)
            .unwrap_or(DEFAULT_SIGHT_RANGE_METERS)
    }

    /// Rebuilds the cached set of npcs `npc` can see: every live npc within
    /// [`sight_range`](Self::sight_range) for which `blocked` reports no static
    /// geometry between the two roots.
    pub fn refresh_visibility(
        &mut self,
        npc: InstanceId,
        blocked: &mut dyn FnMut(Vec3, Vec3) -> bool,
    ) {
        let Some(position) = self.npc_state(npc).and_then(|npc| npc.live.position) else {
            // Without an entity (dead, not spawned) there is nothing to see.
            self.store_seen(npc, HashSet::new());
            return;
        };
        let range = self.sight_range(npc);
        let range_squared = range * range;
        let mut seen = HashSet::new();
        for (&candidate, runtime) in &self.loaded_state.npc_instances {
            if candidate == npc {
                continue;
            }
            let candidate_position = {
                let runtime = runtime.read().unwrap_or_else(PoisonError::into_inner);
                if runtime.live.dead {
                    continue;
                }
                let Some(candidate_position) = runtime.live.position else {
                    continue;
                };
                candidate_position
            };
            if (candidate_position - position).length_squared() > range_squared {
                continue;
            }
            if blocked(position, candidate_position) {
                continue;
            }
            seen.insert(candidate);
        }
        self.store_seen(npc, seen);
    }

    /// Cached answer to whether `npc` sees `other`, `None` when no set was built
    /// for this npc yet.
    pub fn cached_sight(&self, npc: InstanceId, other: InstanceId) -> Option<bool> {
        self.npc_state(npc)?
            .perception
            .seen
            .as_ref()
            .map(|seen| seen.contains(&other))
    }

    /// Whether `other` is inside the npc ±100° view cone (`Npc_CanSeeNpc`).
    /// `true` when the facing or a position is unknown, so the cone never hides
    /// an npc the world side did not describe.
    pub fn in_view_cone(&self, npc: InstanceId, other: InstanceId) -> bool {
        let Some((facing, position)) = self
            .npc_state(npc)
            .and_then(|state| Some((state.live.facing?, state.live.position?)))
        else {
            return true;
        };
        let Some(other_position) = self.npc_state(other).and_then(|state| state.live.position)
        else {
            return true;
        };
        let Some(to_other) = horizontal_offset(position, other_position).try_normalize() else {
            return true;
        };
        facing.dot(to_other) > VIEW_CONE_COS
    }

    /// Whether `npc` sees `other`: the refreshed sight set (with the view cone
    /// unless `free_los`) or, for npcs never scanned, distance only.
    pub fn can_see(&self, npc: InstanceId, other: InstanceId, free_los: bool) -> bool {
        if let Some(seen) = self.cached_sight(npc, other) {
            return seen && (free_los || self.in_view_cone(npc, other));
        }
        let positions = (
            self.npc_state(npc).and_then(|npc| npc.live.position),
            self.npc_state(other).and_then(|npc| npc.live.position),
        );
        let (Some(npc_position), Some(other_position)) = positions else {
            return false;
        };
        (npc_position - other_position).length() <= self.sight_range(npc)
    }

    /// Writes the visibility cache without recording the npc as changed, it is
    /// scratch data the ECS side mirrors nothing from.
    fn store_seen(&mut self, npc: InstanceId, seen: HashSet<InstanceId>) {
        let Some(runtime) = self.loaded_state.npc_instances.get(&npc) else {
            return;
        };
        let mut runtime = runtime.write().unwrap_or_else(PoisonError::into_inner);
        runtime.perception.seen = Some(seen);
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
        let position = self.npc_state(npc)?.live.position?;
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
        self.messages.screen_messages.send(ScreenMessage {
            text,
            x,
            y,
            time_sec: time,
        });
    }

    /// Queues a sound for the ECS bridge. `sound` is a string literal with a
    /// `C_SFX` instance name, uppercased here because the script passes it in
    /// arbitrary case while the definitions are stored uppercase.
    pub(crate) fn push_sound(&mut self, sound: i32, position: Option<Vec3>) {
        let Some(id) = self.symbol_table.string(sound) else {
            warn!("Sound: name({sound}) is not a string");
            return;
        };
        let Some(name) = self.get_string_non_0(id.value()) else {
            warn!("Sound: name({sound}) has no content");
            return;
        };
        self.messages.sounds.send(ScriptSound {
            name: name.to_uppercase(),
            position,
        });
    }

    /// Queues a sound played at the npc's world position (`Snd_Play3D`,
    /// `AI_Snd_Play`). Falls back to global playback when the npc has no world
    /// entity yet.
    pub(crate) fn push_sound_at_npc(&mut self, npc: i32, sound: i32) {
        let Some(npc) = self.symbol_table.instance(npc) else {
            warn!("Sound: npc({npc}) is not an instance");
            return;
        };
        let position = self.npc_state(npc).and_then(|state| state.live.position);
        if position.is_none() {
            warn!(
                "Sound: npc({}) has no world position, playing globally",
                npc.value()
            );
        }
        self.push_sound(sound, position);
    }

    /// Queues an `AI_OutputSVM` line. The name is a `$`-prefixed text key that
    /// the `C_SVM` instance of the speaking voice maps to a speech file.
    pub(crate) fn push_svm_line(&mut self, npc: i32, svm_name: i32, overlay: bool) {
        let Some(npc) = self.symbol_table.instance(npc) else {
            warn!("AI_OutputSVM with invalid npc instance({npc})");
            return;
        };
        let Some(svm_id) = self.symbol_table.string(svm_name) else {
            warn!("AI_OutputSVM with invalid name({svm_name})");
            return;
        };
        let Some(svm) = self.get_string_non_0(svm_id.value()) else {
            return;
        };
        let Some(sound) = self.svm_sound(npc, &svm) else {
            bevy::log::debug!(
                "AI_OutputSVM npc({}) has no C_SVM entry for({svm})",
                npc.value()
            );
            return;
        };
        let position = self.npc_state(npc).and_then(|state| state.live.position);
        self.messages.svm.send(SvmLine {
            speaker: npc,
            sound,
            position,
            overlay,
        });
    }

    /// Resolves a `$`-prefixed SVM key through the `C_SVM` instance of the
    /// npc voice: `$IGETYOUSTILL` + voice 1 -> `SVM_1_IGetYouStill`.
    fn svm_sound(&self, npc: InstanceId, svm: &str) -> Option<String> {
        let key = svm.strip_prefix('$')?;
        let voice = self.typed.npc(npc.value() as u32)?.voice;
        if voice < 0 {
            return None;
        }
        let instance = self
            .symbol_table
            .symbol_id_by_name(&format!("svm_{voice}"))?;
        let member = self
            .symbol_table
            .symbol_id_by_name(&format!("c_svm.{}", key.to_ascii_lowercase()))?;
        let raw = self.typed.read_member(instance as u32, member as u32, 0);
        let string_id = self.symbol_table.string(raw)?;
        self.get_string_non_0(string_id.value())
    }

    /// Queues a chapter splash for the ECS bridge. A newer request replaces an
    /// undrained one, matching the engine which keeps a single pending chapter.
    pub(crate) fn push_chapter_intro(
        &mut self,
        title: i32,
        subtitle: i32,
        texture: i32,
        sound: i32,
        time_ms: i32,
    ) {
        let Some(title) = self.chapter_string(title, "title") else {
            return;
        };
        let Some(subtitle) = self.chapter_string(subtitle, "subtitle") else {
            return;
        };
        let Some(texture) = self.chapter_string(texture, "texture") else {
            return;
        };
        let Some(sound) = self.chapter_string(sound, "sound") else {
            return;
        };
        if self.pending.chapter_intro.is_some() {
            warn!("IntroduceChapter: replacing a chapter screen which was not shown yet");
        }
        self.pending.chapter_intro = Some(ChapterIntro {
            title,
            subtitle,
            texture,
            sound,
            time_ms,
        });
    }

    /// Content of a string argument of `IntroduceChapter`, `None` when the id
    /// does not refer to a string symbol (logged, the screen is dropped).
    fn chapter_string(&self, raw: i32, what: &str) -> Option<String> {
        let Some(id) = self.symbol_table.string(raw) else {
            warn!("IntroduceChapter: {what}({raw}) is not a string, chapter screen dropped");
            return None;
        };
        let Some(value) = self.get_string_non_0(id.value()) else {
            warn!("IntroduceChapter: {what}({raw}) has no content, chapter screen dropped");
            return None;
        };
        Some(value)
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
    pub fn next(&mut self) -> u32 {
        self.rng.next_u32()
    }
}

/// Nearest unoccupied mob to `position` within [`MOB_SEARCH_RADIUS`].
fn nearest_free_mob(mobs: &[ScriptMob], position: Vec3) -> Option<&ScriptMob> {
    let radius_squared = MOB_SEARCH_RADIUS * MOB_SEARCH_RADIUS;
    mobs.iter()
        .filter(|mob| !mob.occupied)
        .map(|mob| (mob, mob.position.distance_squared(position)))
        .filter(|(_, distance)| *distance <= radius_squared)
        .min_by(|(_, left), (_, right)| left.total_cmp(right))
        .map(|(mob, _)| mob)
}

#[cfg(test)]
mod tests {
    use super::instances::{NpcLiveState, NpcRoutineState};
    use super::*;

    /// The guest ring cursor keeps counting past the 64 ring slots, the mirror
    /// must wrap the same way instead of writing every later string to slot 0
    #[test]
    fn dynamic_string_ring_wraps_like_the_guest_cursor() {
        let strings = vec!["symbol".to_owned(), "first_literal".to_owned()];
        let mut table = SymbolTable::new(HashMap::new(), 1, strings);
        let base = 2;

        for index in 0..(DYN_STRINGS_SLOTS_NUM + 5) {
            table.record_dynamic_string(index.to_string().into_bytes());
        }

        // Newest string landed in ring slot `recorded % SLOTS_NUM`
        let newest = DYN_STRINGS_SLOTS_NUM + 4;
        let slot = newest % DYN_STRINGS_SLOTS_NUM;
        assert_eq!(
            table.string_content((base + slot) as i32),
            Some(newest.to_string())
        );
    }

    #[test]
    fn nearest_free_mob_skips_occupied_and_distant_mobs() {
        let mobs = [
            ScriptMob {
                position: Vec3::new(2.0, 0.0, 0.0),
                occupied: true,
                state: 0,
            },
            ScriptMob {
                position: Vec3::new(3.0, 0.0, 0.0),
                occupied: false,
                state: 1,
            },
            ScriptMob {
                position: Vec3::new(20.0, 0.0, 0.0),
                occupied: false,
                state: 0,
            },
        ];

        let mob = nearest_free_mob(&mobs, Vec3::ZERO).expect("a free mob is in range");

        assert_eq!(mob.state, 1);
    }

    #[test]
    fn container_item_count_resolves_by_name_and_item() {
        let mut state = ImportCallState::default();
        let gold = InstanceId::new(10).expect("valid id");
        let bread = InstanceId::new(11).expect("valid id");
        state
            .containers
            .insert("chest_lobart".to_string(), vec![(gold, 2), (bread, 1)]);

        assert_eq!(state.container_item_count("chest_lobart", gold), 2);
        assert_eq!(state.container_item_count("chest_lobart", bread), 1);
        assert_eq!(
            state.container_item_count("chest_lobart", InstanceId::new(12).unwrap()),
            0
        );
        assert_eq!(state.container_item_count("chest_rukhars", gold), 0);
    }

    #[test]
    fn nearest_free_mob_is_none_without_a_mob_in_range() {
        let mobs = [ScriptMob {
            position: Vec3::new(20.0, 0.0, 0.0),
            occupied: false,
            state: 0,
        }];

        assert!(nearest_free_mob(&mobs, Vec3::ZERO).is_none());
    }

    fn insert_npc(
        state: &mut ImportCallState,
        id: i32,
        position: Vec3,
        dead: bool,
        current_state: Option<FunctionId>,
    ) -> InstanceId {
        let id = InstanceId::new(id).expect("valid id");
        state.loaded_state.npc_instances.insert(
            id,
            Arc::new(RwLock::new(NpcRuntime {
                live: NpcLiveState {
                    position: Some(position),
                    dead,
                    ..Default::default()
                },
                routine: NpcRoutineState {
                    current_state,
                    ..Default::default()
                },
                ..Default::default()
            })),
        );
        id
    }

    #[test]
    fn nearest_npc_picks_the_closest_live_match() {
        let mut state = ImportCallState::default();
        let source = insert_npc(&mut state, 1, Vec3::ZERO, false, None);
        insert_npc(&mut state, 2, Vec3::new(5.0, 0.0, 0.0), false, None);
        let near = insert_npc(&mut state, 3, Vec3::new(2.0, 0.0, 0.0), false, None);
        insert_npc(&mut state, 4, Vec3::new(1.0, 0.0, 0.0), true, None);

        let found = state.nearest_npc(source, 10.0, None, None, None, false);

        assert_eq!(found, Some(near));
    }

    #[test]
    fn nearest_npc_applies_instance_state_and_radius_filters() {
        let mut state = ImportCallState::default();
        let source = insert_npc(&mut state, 1, Vec3::ZERO, false, None);
        let state_fn = FunctionId::new(10).expect("valid id");
        let other_state = FunctionId::new(11).expect("valid id");
        let matching = insert_npc(
            &mut state,
            2,
            Vec3::new(3.0, 0.0, 0.0),
            false,
            Some(state_fn),
        );
        let closest = insert_npc(
            &mut state,
            3,
            Vec3::new(1.0, 0.0, 0.0),
            false,
            Some(other_state),
        );
        insert_npc(
            &mut state,
            4,
            Vec3::new(50.0, 0.0, 0.0),
            false,
            Some(state_fn),
        );

        assert_eq!(
            state.nearest_npc(source, 10.0, None, Some(state_fn), None, false),
            Some(matching)
        );
        assert_eq!(
            state.nearest_npc(source, 10.0, Some(closest), None, None, false),
            Some(closest)
        );
        assert_eq!(
            state.nearest_npc(source, 0.5, None, None, None, false),
            None
        );
        assert_eq!(
            state.nearest_npc(source, 10.0, None, None, None, false),
            Some(closest)
        );
    }

    #[test]
    fn nearest_npc_can_exclude_the_player() {
        let mut state = ImportCallState::default();
        let source = insert_npc(&mut state, 1, Vec3::ZERO, false, None);
        let player = insert_npc(&mut state, 2, Vec3::new(1.0, 0.0, 0.0), false, None);
        let npc = insert_npc(&mut state, 3, Vec3::new(4.0, 0.0, 0.0), false, None);
        state.player_instance = Some(player);

        assert_eq!(
            state.nearest_npc(source, 10.0, None, None, None, false),
            Some(player)
        );
        assert_eq!(
            state.nearest_npc(source, 10.0, None, None, None, true),
            Some(npc)
        );
    }

    #[test]
    fn nearest_npc_breaks_ties_by_instance_id() {
        let mut state = ImportCallState::default();
        let source = insert_npc(&mut state, 1, Vec3::ZERO, false, None);
        let low = insert_npc(&mut state, 2, Vec3::new(0.0, 0.0, 3.0), false, None);
        insert_npc(&mut state, 3, Vec3::new(3.0, 0.0, 0.0), false, None);

        assert_eq!(
            state.nearest_npc(source, 10.0, None, None, None, false),
            Some(low)
        );
    }

    #[test]
    fn freepoint_selection_is_nearest_free_and_can_skip_the_current_point() {
        let mut state = ImportCallState::default();
        let npc = InstanceId::new(1).expect("valid id");
        let other = InstanceId::new(2).expect("valid id");
        state.loaded_state.npc_instances.insert(
            npc,
            Arc::new(RwLock::new(NpcRuntime {
                live: NpcLiveState {
                    position: Some(Vec3::ZERO),
                    ..Default::default()
                },
                ..Default::default()
            })),
        );
        state.freepoints = vec![
            ScriptFreePoint {
                name: "fp_near_a".to_string(),
                position: Vec3::new(1.0, 0.0, 0.0),
            },
            ScriptFreePoint {
                name: "fp_near_b".to_string(),
                position: Vec3::new(3.0, 0.0, 0.0),
            },
            ScriptFreePoint {
                name: "fp_far".to_string(),
                position: Vec3::new(50.0, 0.0, 0.0),
            },
        ];

        let nearest = state
            .available_freepoint(npc, "fp_", false)
            .map(|point| point.name.clone());
        assert_eq!(nearest.as_deref(), Some("fp_near_a"));

        state.freepoint_users.insert("fp_near_a".to_string(), other);
        let nearest = state
            .available_freepoint(npc, "fp_", false)
            .map(|point| point.name.clone());
        assert_eq!(nearest.as_deref(), Some("fp_near_b"));

        state.assign_freepoint(npc, "fp_near_b");
        assert!(state.available_freepoint(npc, "fp_", true).is_none());
        assert!(state.available_freepoint(npc, "fp_", false).is_some());
    }

    #[test]
    fn enemy_selection_uses_the_attitude_toward_the_player() {
        use crate::zengin::script::host::{ATT_FRIENDLY, ATT_HOSTILE};

        let mut state = ImportCallState::default();
        let npc = insert_npc(&mut state, 1, Vec3::ZERO, false, None);
        let player = insert_npc(&mut state, 2, Vec3::new(5.0, 0.0, 0.0), false, None);
        state.player_instance = Some(player);
        state.perc_ranges.insert(PERC_ASSESSENEMY, 2000);

        state.npc_state_mut(npc).social.attitude = Some(ATT_HOSTILE);
        assert_eq!(state.nearest_enemy(npc), Some(player));

        state.npc_state_mut(npc).social.attitude = Some(ATT_FRIENDLY);
        assert_eq!(state.nearest_enemy(npc), None);
    }

    #[test]
    fn enemy_selection_skips_dead_and_distant_npcs() {
        use crate::zengin::script::host::ATT_HOSTILE;

        let mut state = ImportCallState::default();
        let npc = insert_npc(&mut state, 1, Vec3::ZERO, false, None);
        let player = insert_npc(&mut state, 2, Vec3::new(50.0, 0.0, 0.0), false, None);
        state.player_instance = Some(player);
        state.perc_ranges.insert(PERC_ASSESSENEMY, 1000);
        state.npc_state_mut(npc).social.attitude = Some(ATT_HOSTILE);

        assert_eq!(state.nearest_enemy(npc), None);

        state.npc_state_mut(player).live.position = Some(Vec3::new(5.0, 0.0, 0.0));
        assert_eq!(state.nearest_enemy(npc), Some(player));

        state.npc_state_mut(player).live.dead = true;
        assert_eq!(state.nearest_enemy(npc), None);
    }

    #[test]
    fn enemy_selection_uses_the_cached_sight_set() {
        use crate::zengin::script::host::ATT_HOSTILE;

        let mut state = ImportCallState::default();
        let npc = insert_npc(&mut state, 1, Vec3::ZERO, false, None);
        let player = insert_npc(&mut state, 2, Vec3::new(5.0, 0.0, 0.0), false, None);
        state.player_instance = Some(player);
        state.perc_ranges.insert(PERC_ASSESSENEMY, 2000);
        state.npc_state_mut(npc).social.attitude = Some(ATT_HOSTILE);

        state.npc_state_mut(npc).perception.seen = Some(HashSet::new());
        assert_eq!(state.nearest_enemy(npc), None);

        state.npc_state_mut(npc).perception.seen = Some(HashSet::from([player]));
        assert_eq!(state.nearest_enemy(npc), Some(player));

        // No cache: an npc away from the player falls back to distance only.
        state.npc_state_mut(npc).perception.seen = None;
        assert_eq!(state.nearest_enemy(npc), Some(player));
    }

    #[test]
    fn visibility_refresh_applies_the_sight_test_and_range() {
        let mut state = ImportCallState::default();
        let npc = insert_npc(&mut state, 1, Vec3::ZERO, false, None);
        let behind_wall = insert_npc(&mut state, 2, Vec3::new(-3.0, 0.0, 0.0), false, None);
        let in_sight = insert_npc(&mut state, 3, Vec3::new(3.0, 0.0, 0.0), false, None);
        let far = insert_npc(&mut state, 4, Vec3::new(100.0, 0.0, 0.0), false, None);

        // A wall at x = 0.
        state.refresh_visibility(npc, &mut |_, target| target.x < 0.0);

        assert_eq!(state.cached_sight(npc, in_sight), Some(true));
        assert_eq!(state.cached_sight(npc, behind_wall), Some(false));
        assert_eq!(state.cached_sight(npc, far), Some(false));
    }

    #[test]
    fn visibility_refresh_skips_dead_and_positionless_npcs() {
        let mut state = ImportCallState::default();
        let npc = insert_npc(&mut state, 1, Vec3::ZERO, false, None);
        let dead = insert_npc(&mut state, 2, Vec3::new(1.0, 0.0, 0.0), true, None);
        let unspawned = insert_npc(&mut state, 3, Vec3::new(2.0, 0.0, 0.0), false, None);
        state.npc_state_mut(unspawned).live.position = None;

        state.refresh_visibility(npc, &mut |_, _| false);

        assert_eq!(state.cached_sight(npc, dead), Some(false));
        assert_eq!(state.cached_sight(npc, unspawned), Some(false));
    }

    #[test]
    fn view_cone_limits_non_free_sight() {
        let mut state = ImportCallState::default();
        let npc = insert_npc(&mut state, 1, Vec3::ZERO, false, None);
        let front = insert_npc(&mut state, 2, Vec3::new(0.0, 0.0, 3.0), false, None);
        let behind = insert_npc(&mut state, 3, Vec3::new(0.0, 0.0, -3.0), false, None);
        state.npc_state_mut(npc).live.facing = Some(Vec3::Z);

        assert!(state.in_view_cone(npc, front));
        assert!(!state.in_view_cone(npc, behind));

        // Unknown facing: the cone never hides the target.
        state.npc_state_mut(npc).live.facing = None;
        assert!(state.in_view_cone(npc, behind));
    }

    #[test]
    fn can_see_applies_the_cone_only_without_free_los() {
        let mut state = ImportCallState::default();
        let npc = insert_npc(&mut state, 1, Vec3::ZERO, false, None);
        let behind = insert_npc(&mut state, 2, Vec3::new(0.0, 0.0, -3.0), false, None);
        state.npc_state_mut(npc).live.facing = Some(Vec3::Z);
        state.npc_state_mut(npc).perception.seen = Some(HashSet::from([behind]));

        assert!(!state.can_see(npc, behind, false));
        assert!(state.can_see(npc, behind, true));
    }

    #[test]
    fn guild_attitude_table_is_read_as_a_square_matrix() {
        use daedalus_typed::{SymbolDecl, SymbolKind};

        let mut registry = TypedRegistry::new();
        let symbol = registry.add_symbol(SymbolDecl {
            name: "gil_attitudes",
            kind: SymbolKind::Int,
            is_const: true,
            is_class_member: false,
            address_or_offset: 0,
            element_count: 4,
            has_function: false,
            parent: None,
        });
        registry.finish();
        registry.write_values(symbol, &[0, 3, 2, 3]);

        let mut state = ImportCallState {
            typed: registry,
            ..Default::default()
        };
        state.exchange_guild_attitudes(symbol);

        assert_eq!(state.guild_attitudes.get(&(0, 1)), Some(&3));
        assert_eq!(state.guild_attitudes.get(&(1, 0)), Some(&2));
        assert_eq!(state.guild_attitudes.len(), 4);
    }
}
