//! Bridges changes produced inside the script vm to the ECS. The script appends
//! change records to its state, `send_script_changes` drains them into Bevy
//! messages and the matching `apply_*` systems turn them into component changes
//! on the affected entities.

use bevy::prelude::*;

use crate::game_state::AppState;
use crate::gameplay::game_time::{GameTime, advance_game_time};
use crate::zengin::GameNpc;
use crate::zengin::PlayerCharacter;
use crate::zengin::gui::GameLogEntry;
use crate::zengin::instance_index::{InstanceIndex, InstanceIndexUpdated};
use crate::zengin::npc_routines::{
    NpcArmorItem, NpcEquippedWeapon, NpcHealth, NpcLookAt, NpcOverlay, NpcTalentOverlay,
    NpcWalkMode, NpcWeaponMode, overlay_scheme, talent_overlay_for,
};
use crate::zengin::script::component::ScriptResource;
use crate::zengin::script::journal::{JournalAction, JournalChange, JournalSection, JournalStatus};
use crate::zengin::script::state::{
    AlignToWpChange, AnimationChange, AttackChange, ChapterIntro, ClearAiQueueChange, DialogClose,
    DialogLine, GotoChange, GotoFpChange, GotoNpcChange, InventoryGain, ItemChanged,
    ItemEntitiesChanged, NpcChanged, NpcEntitiesChanged, OverlayChange, OverlayRequest,
    PassivePerception, PrintLine, ProcessInfosChange, RoutineEntry, ScreenMessage, ScriptMessages,
    ScriptSound, SetGameTime, StandUpChange, StartStateChange, SvmLine, TurnToNpcChange,
    UseItemStateChange, UseMobChange, WaitChange, WeaponMode,
};
use crate::zengin::world_spawn::WorldSpawnMessages;
use dev_util::external_messages::{ExternalMessageAppExt, pump_external_messages};
use dev_util::perf_debug;

pub struct ScriptBridgePlugin;

impl Plugin for ScriptBridgePlugin {
    fn build(&self, app: &mut App) {
        // The world loader and the script share one `SpawnFromSavedObject`
        // queue; the ECS inbox is keyed by message type, so a second
        // `add_external_message` would orphan this sender.
        let spawn_objects = app.world().resource::<WorldSpawnMessages>().objects.clone();
        let messages = ScriptMessages {
            align_to_wp: app.add_external_message::<AlignToWpChange>(),
            overlay: app.add_external_message::<OverlayChange>(),
            goto: app.add_external_message::<GotoChange>(),
            goto_fp: app.add_external_message::<GotoFpChange>(),
            goto_npc: app.add_external_message::<GotoNpcChange>(),
            attack: app.add_external_message::<AttackChange>(),
            turn_to_npc: app.add_external_message::<TurnToNpcChange>(),
            process_infos: app.add_external_message::<ProcessInfosChange>(),
            animation: app.add_external_message::<AnimationChange>(),
            start_state: app.add_external_message::<StartStateChange>(),
            use_mob: app.add_external_message::<UseMobChange>(),
            use_item_state: app.add_external_message::<UseItemStateChange>(),
            wait: app.add_external_message::<WaitChange>(),
            stand_up: app.add_external_message::<StandUpChange>(),
            clear_ai_queue: app.add_external_message::<ClearAiQueueChange>(),
            spawn_objects,
            dialog_lines: app.add_external_message::<DialogLine>(),
            inventory_gains: app.add_external_message::<InventoryGain>(),
            screen_messages: app.add_external_message::<ScreenMessage>(),
            print_lines: app.add_external_message::<PrintLine>(),
            sounds: app.add_external_message::<ScriptSound>(),
            svm: app.add_external_message::<SvmLine>(),
            passive_perception: app.add_external_message::<PassivePerception>(),
            journal_changes: app.add_external_message::<JournalChange>(),
            game_time: app.add_external_message::<SetGameTime>(),
        };
        app.insert_resource(messages);

        app.add_message::<NpcChanged>();
        app.add_message::<NpcEntitiesChanged>();
        app.add_message::<ItemChanged>();
        app.add_message::<ItemEntitiesChanged>();
        app.add_message::<DialogClose>();
        app.add_message::<ChapterIntro>();
        app.add_systems(
            Update,
            (
                pump_external_messages::<AlignToWpChange>,
                pump_external_messages::<GotoChange>,
                pump_external_messages::<GotoFpChange>,
                pump_external_messages::<GotoNpcChange>,
                pump_external_messages::<AttackChange>,
                pump_external_messages::<TurnToNpcChange>,
                pump_external_messages::<UseMobChange>,
                pump_external_messages::<UseItemStateChange>,
                pump_external_messages::<WaitChange>,
                pump_external_messages::<StandUpChange>,
            )
                .in_set(ScriptOutputFlush),
        );
        app.add_systems(
            Update,
            (
                sync_script_clock,
                apply_script_requests,
                send_npc_changes,
                expand_npc_changes,
                send_item_changes,
                expand_item_changes,
                log_inventory_gains,
                log_print_lines,
                log_journal_changes,
                forward_chapter_intro,
                sync_npc_routine_entry,
                sync_npc_look_at,
                sync_npc_weapon_mode,
                sync_npc_walk_mode,
                sync_npc_equipped_weapon,
                sync_npc_armor,
                sync_npc_health,
                sync_npc_talent_overlay,
                apply_overlay_changes,
                seed_npc_components_on_add,
            )
                .chain()
                .in_set(ScriptChangesApplied)
                .after(advance_game_time)
                .after(InstanceIndexUpdated)
                .run_if(in_state(AppState::InGame)),
        );
    }
}

/// Marks systems that have applied script produced changes to the ECS. Systems
/// reacting to those changes run after this set.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ScriptChangesApplied;

/// Copies host requests queued during the current update into the message
/// buffers, so action consumers later in the same update see them.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ScriptOutputFlush;

/// Exposes the running clock to the script host (`Wld_GetDay` and the refuse
/// talk deadline).
fn sync_script_clock(
    time: Res<Time>,
    game_time: Res<GameTime>,
    gothic_script: Res<ScriptResource>,
) {
    let _perf = perf_debug!();
    let mut script = gothic_script.lock();
    script.set_game_day(game_time.day.saturating_sub(1));
    script.set_game_minutes_of_day(game_time.minutes_of_day());
    script.set_world_seconds(time.elapsed_secs_f64());
}

/// Applies the script requests that cannot be a plain message: closing the
/// dialog flips a flag and `Npc_ExchangeRoutine` has to call back into the vm.
///
/// `Npc_ExchangeRoutine` cannot run its `Rtn_*` function from inside the host
/// import (wasmtime traps on re-entrancy), so it is resolved here: the function
/// refills the npc's routine entries.
fn apply_script_requests(
    mut dialog_close: MessageWriter<DialogClose>,
    gothic_script: Res<ScriptResource>,
) {
    let _perf = perf_debug!();
    let mut script = gothic_script.lock();
    if script.take_close_dialog() {
        dialog_close.write(DialogClose);
    }
    for exchange in script.drain_routine_exchanges() {
        script.exchange_routine(exchange.instance_id, &exchange.routine_name);
    }
}

/// Drains npc data edits recorded by the script and forwards them as messages.
fn send_npc_changes(mut changed: MessageWriter<NpcChanged>, gothic_script: Res<ScriptResource>) {
    let _perf = perf_debug!();
    let changed_ids = gothic_script.lock().drain_npc_changes();
    if changed_ids.is_empty() {
        return;
    }
    debug!("Npc data changed: {} instance(s)", changed_ids.len());
    for instance_id in changed_ids {
        changed.write(NpcChanged { instance_id });
    }
}

/// Resolves the entities carrying a changed npc instance.
fn expand_npc_changes(
    mut changes: MessageReader<NpcChanged>,
    mut entities_changed: MessageWriter<NpcEntitiesChanged>,
    index: Res<InstanceIndex>,
) {
    let _perf = perf_debug!();
    for change in changes.read() {
        let Some(handle) = index.npc_handle(change.instance_id) else {
            continue;
        };
        let entities = index.npcs(change.instance_id);
        if entities.is_empty() {
            continue;
        }
        entities_changed.write(NpcEntitiesChanged {
            handle: handle.clone(),
            entities: entities.to_vec(),
        });
    }
}

/// Drains item data edits recorded by the script and forwards them as messages.
fn send_item_changes(mut changed: MessageWriter<ItemChanged>, gothic_script: Res<ScriptResource>) {
    let _perf = perf_debug!();
    let mut script = gothic_script.lock();
    let changed_ids = script.drain_item_changes();
    if changed_ids.is_empty() {
        return;
    }
    debug!("Item data changed: {} instance(s)", changed_ids.len());
    for instance_id in changed_ids {
        // Resolve the typed members into the runtime strings the consumers read.
        script.refresh_item_runtime(instance_id);
        changed.write(ItemChanged { instance_id });
    }
}

/// Resolves the entities carrying a changed item instance.
fn expand_item_changes(
    mut changes: MessageReader<ItemChanged>,
    mut entities_changed: MessageWriter<ItemEntitiesChanged>,
    index: Res<InstanceIndex>,
) {
    let _perf = perf_debug!();
    for change in changes.read() {
        let Some(handle) = index.item_handle(change.instance_id) else {
            continue;
        };
        let entities = index.items(change.instance_id);
        if entities.is_empty() {
            continue;
        }
        entities_changed.write(ItemEntitiesChanged {
            handle: handle.clone(),
            entities: entities.to_vec(),
        });
    }
}

/// Forwards the chapter splash the script requested as an ECS message.
fn forward_chapter_intro(
    mut chapter_intro: MessageWriter<ChapterIntro>,
    gothic_script: Res<ScriptResource>,
) {
    let _perf = perf_debug!();
    let mut script = gothic_script.lock();
    if let Some(intro) = script.drain_chapter_intro() {
        chapter_intro.write(intro);
    }
}

/// Shows the items the player's inventory gained through the script in the
/// bottom-left game log.
fn log_inventory_gains(
    mut gains: MessageReader<InventoryGain>,
    mut game_log: MessageWriter<GameLogEntry>,
    gothic_script: Res<ScriptResource>,
) {
    let _perf = perf_debug!();
    if gains.is_empty() {
        return;
    }
    let mut script = gothic_script.lock();
    for gain in gains.read() {
        let name = script.item_log_name(gain.item);
        game_log.write(GameLogEntry {
            text: format!("{name} x{}", gain.count),
        });
        info!(
            "Player({}) received item({}) x{}",
            gain.npc.value(),
            name,
            gain.count
        );
    }
}

fn log_print_lines(mut lines: MessageReader<PrintLine>, mut game_log: MessageWriter<GameLogEntry>) {
    let _perf = perf_debug!();
    for line in lines.read() {
        game_log.write(GameLogEntry {
            text: line.text.clone(),
        });
    }
}

/// Shows the journal (quest log) changes the script made in the bottom-left
/// game log.
fn log_journal_changes(
    mut changes: MessageReader<JournalChange>,
    mut game_log: MessageWriter<GameLogEntry>,
) {
    let _perf = perf_debug!();
    for change in changes.read() {
        let action = match &change.action {
            JournalAction::Created { section } => format!(
                "created, section {}",
                match section {
                    JournalSection::Mission => "mission",
                    JournalSection::Note => "note",
                }
            ),
            JournalAction::Status(status) => format!(
                "status {}",
                match status {
                    JournalStatus::Running => "running",
                    JournalStatus::Success => "success",
                    JournalStatus::Failed => "failed",
                    JournalStatus::Obsolete => "obsolete",
                }
            ),
            JournalAction::Entry(entry) => format!("entry: {entry}"),
        };
        game_log.write(GameLogEntry {
            text: format!("Journal: {} — {action}", change.topic),
        });
    }
}

/// Refreshes the active routine entry on the entities of a changed npc. The
/// entry is derived from the current minute, the same way the minute-change
/// seeder does it.
fn sync_npc_routine_entry(
    mut commands: Commands,
    mut changes: MessageReader<NpcEntitiesChanged>,
    game_time: Res<GameTime>,
    npcs: Query<&RoutineEntry>,
) {
    let _perf = perf_debug!();
    let minutes = game_time.minutes_of_day();
    for change in changes.read() {
        let active = change
            .handle
            .runtime()
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get_routine_entry(minutes)
            .cloned();
        for &entity in &change.entities {
            let current = npcs.get(entity).ok();
            let needs_update = match (&active, current) {
                (Some(active), Some(current)) => active != current,
                (None, None) => false,
                _ => true,
            };
            if !needs_update {
                continue;
            }
            set_routine_entry(&mut commands, entity, active.as_ref());
        }
    }
}

/// Inserts the active routine entry on the entity, or removes the component when
/// the instance has no routine entry active. Index tracked entities may have
/// been despawned earlier in the same frame, so the commands must not panic on
/// despawned entities.
fn set_routine_entry(commands: &mut Commands, entity: Entity, active: Option<&RoutineEntry>) {
    match active {
        Some(active) => {
            commands.entity(entity).try_insert(active.clone());
        }
        None => {
            commands.entity(entity).try_remove::<RoutineEntry>();
        }
    }
}

/// Refreshes the look-at target of the entities of a changed npc.
fn sync_npc_look_at(
    mut commands: Commands,
    mut changes: MessageReader<NpcEntitiesChanged>,
    index: Res<InstanceIndex>,
    npcs: Query<&NpcLookAt>,
) {
    let _perf = perf_debug!();
    for change in changes.read() {
        let target = change
            .handle
            .runtime()
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .live
            .look_at
            .and_then(|id| index.npcs(id).first().copied());
        for &entity in &change.entities {
            if npcs.get(entity).ok().map(|look_at| look_at.0) == target {
                continue;
            }
            set_look_at(&mut commands, entity, target);
        }
    }
}

/// Inserts the look-at target entity on the entity, or removes the component
/// when the npc looks at nothing.
fn set_look_at(commands: &mut Commands, entity: Entity, target: Option<Entity>) {
    match target {
        Some(target) => {
            commands.entity(entity).try_insert(NpcLookAt(target));
        }
        None => {
            commands.entity(entity).try_remove::<NpcLookAt>();
        }
    }
}

/// Refreshes the weapon mode component of the entities of a changed npc.
fn sync_npc_weapon_mode(
    mut commands: Commands,
    mut changes: MessageReader<NpcEntitiesChanged>,
    npcs: Query<&NpcWeaponMode>,
) {
    let _perf = perf_debug!();
    for change in changes.read() {
        let mode = change
            .handle
            .runtime()
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .equipment
            .weapon_mode;
        for &entity in &change.entities {
            if npcs.get(entity).ok().map(|current| current.0) == mode {
                continue;
            }
            set_weapon_mode(&mut commands, entity, mode);
        }
    }
}

/// Refreshes the walk mode component of the entities of a changed npc.
fn sync_npc_walk_mode(
    mut commands: Commands,
    mut changes: MessageReader<NpcEntitiesChanged>,
    npcs: Query<&NpcWalkMode>,
) {
    let _perf = perf_debug!();
    for change in changes.read() {
        let mode = change
            .handle
            .runtime()
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .live
            .walk_mode;
        for &entity in &change.entities {
            if npcs.get(entity).is_ok_and(|current| current.0 == mode) {
                continue;
            }
            commands.entity(entity).try_insert(NpcWalkMode(mode));
        }
    }
}

/// Inserts the weapon mode on the entity, or removes the component when the npc
/// has no weapon mode.
fn set_weapon_mode(commands: &mut Commands, entity: Entity, mode: Option<WeaponMode>) {
    match mode {
        Some(mode) => {
            commands.entity(entity).try_insert(NpcWeaponMode(mode));
        }
        None => {
            commands.entity(entity).try_remove::<NpcWeaponMode>();
        }
    }
}

/// Refreshes the equipped weapons component of the entities of a changed npc.
fn sync_npc_equipped_weapon(
    mut commands: Commands,
    mut changes: MessageReader<NpcEntitiesChanged>,
    npcs: Query<&NpcEquippedWeapon>,
    gothic_script: Res<ScriptResource>,
) {
    let _perf = perf_debug!();
    for change in changes.read() {
        let (melee, ranged) = {
            let runtime = change
                .handle
                .runtime()
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            (runtime.equipment.melee, runtime.equipment.ranged)
        };
        let melee_class = melee.and_then(|item| gothic_script.lock().item_melee_class(item));
        let equipped = (melee.is_some() || ranged.is_some()).then_some(NpcEquippedWeapon {
            melee,
            ranged,
            melee_class,
        });
        for &entity in &change.entities {
            let needs_update = match (&equipped, npcs.get(entity).ok()) {
                (Some(equipped), Some(current)) => {
                    equipped.melee != current.melee
                        || equipped.ranged != current.ranged
                        || equipped.melee_class != current.melee_class
                }
                (None, None) => false,
                _ => true,
            };
            if !needs_update {
                continue;
            }
            set_equipped_weapon(&mut commands, entity, equipped.clone());
        }
    }
}

/// Refreshes the armor component of the entities of a changed npc.
fn sync_npc_armor(
    mut commands: Commands,
    mut changes: MessageReader<NpcEntitiesChanged>,
    npcs: Query<&NpcArmorItem>,
) {
    let _perf = perf_debug!();
    for change in changes.read() {
        let armor = change
            .handle
            .runtime()
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .equipment
            .armor;
        for &entity in &change.entities {
            if npcs.get(entity).ok().map(|current| current.0) == Some(armor) {
                continue;
            }
            commands.entity(entity).try_insert(NpcArmorItem(armor));
        }
    }
}

/// Inserts the equipped weapons on the entity, or removes the component when
/// the npc has none equipped.
fn set_equipped_weapon(
    commands: &mut Commands,
    entity: Entity,
    equipped: Option<NpcEquippedWeapon>,
) {
    match equipped {
        Some(equipped) => {
            commands.entity(entity).try_insert(equipped);
        }
        None => {
            commands.entity(entity).try_remove::<NpcEquippedWeapon>();
        }
    }
}

/// Refreshes the weapon skill overlay of the player's entities. Npc combat
/// keeps its FAI driven swing variety, so only the player mirrors the talent.
fn sync_npc_talent_overlay(
    mut commands: Commands,
    mut changes: MessageReader<NpcEntitiesChanged>,
    players: Query<&NpcTalentOverlay, With<PlayerCharacter>>,
) {
    let _perf = perf_debug!();
    for change in changes.read() {
        let mut player_entities = change
            .entities
            .iter()
            .copied()
            .filter(|entity| players.contains(*entity));
        let Some(first) = player_entities.next() else {
            continue;
        };
        let overlay = talent_overlay_for(&change.handle);
        for entity in std::iter::once(first).chain(player_entities) {
            let Ok(current) = players.get(entity) else {
                continue;
            };
            if *current == overlay {
                continue;
            }
            commands.entity(entity).try_insert(overlay.clone());
        }
    }
}

/// Refreshes the hitpoints component of the entities of a changed npc.
fn sync_npc_health(
    mut commands: Commands,
    mut changes: MessageReader<NpcEntitiesChanged>,
    npcs: Query<&NpcHealth>,
) {
    let _perf = perf_debug!();
    for change in changes.read() {
        let (hitpoints, hitpoints_max) = {
            let typed = change
                .handle
                .typed()
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            (typed.hitpoints, typed.hitpoints_max)
        };
        for &entity in &change.entities {
            let needs_update = npcs.get(entity).map_or(true, |current| {
                current.hitpoints != hitpoints || current.hitpoints_max != hitpoints_max
            });
            if !needs_update {
                continue;
            }
            commands
                .entity(entity)
                .try_insert(NpcHealth::new(hitpoints, hitpoints_max));
        }
    }
}

/// Applies overlay change requests to the entities carrying the affected
/// instance ids, writing the [`NpcOverlay`] component. The overlay symbol name
/// is resolved through the script vm, which is only safe outside a running
/// script, hence this runs in `Update`.
fn apply_overlay_changes(
    mut commands: Commands,
    mut changes: MessageReader<OverlayChange>,
    index: Res<InstanceIndex>,
    gothic_script: Res<ScriptResource>,
) {
    let _perf = perf_debug!();
    if changes.is_empty() {
        return;
    }

    let mut gothic_script = gothic_script.lock();
    for change in changes.read() {
        let scheme = match change.request {
            OverlayRequest::Apply(id) => gothic_script
                .get_name_by_id(id)
                .map(|name| overlay_scheme(&name)),
            OverlayRequest::Remove => None,
        };
        for entity in index.npcs(change.instance_id) {
            commands
                .entity(*entity)
                .try_insert(NpcOverlay(scheme.clone()));
        }
    }
}

/// Seeds all script driven npc components on newly spawned npcs. Change messages
/// are one-shot, a runtime spawned npc never sees the messages that were
/// produced before it existed, so it catches up from the current shared state.
fn seed_npc_components_on_add(
    mut commands: Commands,
    added_npcs: Query<(Entity, &GameNpc), Added<GameNpc>>,
    index: Res<InstanceIndex>,
    game_time: Res<GameTime>,
    gothic_script: Res<ScriptResource>,
) {
    let _perf = perf_debug!();
    if added_npcs.is_empty() {
        return;
    }

    let minutes = game_time.minutes_of_day();
    for (entity, npc) in &added_npcs {
        let (hitpoints, hitpoints_max) = {
            let typed = npc
                .handle()
                .typed()
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            (typed.hitpoints, typed.hitpoints_max)
        };
        commands
            .entity(entity)
            .try_insert(NpcHealth::new(hitpoints, hitpoints_max));

        let (melee, ranged) = {
            let instance = npc
                .handle()
                .runtime()
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            set_routine_entry(&mut commands, entity, instance.get_routine_entry(minutes));
            let target = instance
                .live
                .look_at
                .and_then(|id| index.npcs(id).first().copied());
            set_look_at(&mut commands, entity, target);
            set_weapon_mode(&mut commands, entity, instance.equipment.weapon_mode);
            commands
                .entity(entity)
                .try_insert(NpcWalkMode(instance.live.walk_mode));
            commands
                .entity(entity)
                .try_insert(NpcArmorItem(instance.equipment.armor));
            (instance.equipment.melee, instance.equipment.ranged)
        };
        let melee_class = melee.and_then(|item| gothic_script.lock().item_melee_class(item));
        let equipped = (melee.is_some() || ranged.is_some()).then_some(NpcEquippedWeapon {
            melee,
            ranged,
            melee_class,
        });
        set_equipped_weapon(&mut commands, entity, equipped);
    }
}
