//! Bridges changes produced inside the script vm to the ECS. The script appends
//! change records to its state, `send_script_changes` drains them into Bevy
//! messages and the matching `apply_*` systems turn them into component changes
//! on the affected entities.

use std::collections::HashMap;

use bevy::prelude::*;

use crate::gameplay::game_time::{GameTime, advance_game_time};
use crate::gui::GameLogEntry;
use crate::zengin::GameNpc;
use crate::zengin::instance_index::{InstanceIndex, InstanceIndexUpdated};
use crate::zengin::npc_routines::{
    NpcEquippedWeapon, NpcHealth, NpcLookAt, NpcOverlay, NpcWeaponMode, overlay_scheme,
};
use crate::zengin::script::component::ScriptResource;
use crate::zengin::script::journal::{JournalAction, JournalChange, JournalSection, JournalStatus};
use crate::zengin::script::state::{
    AlignToWpChange, DialogClose, DialogLine, EquipItemChange, GotoChange, HealthChange,
    InventoryGain, NpcLookAtChange, OverlayChange, OverlayRequest, RoutineEntry,
    RoutineEntryChange, ScreenMessage, SpawnItem, SpawnNpc, WeaponMode, WeaponModeChange,
};

pub struct ScriptBridgePlugin;

impl Plugin for ScriptBridgePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<RoutineEntryChange>();
        app.add_message::<NpcLookAtChange>();
        app.add_message::<WeaponModeChange>();
        app.add_message::<EquipItemChange>();
        app.add_message::<HealthChange>();
        app.add_message::<AlignToWpChange>();
        app.add_message::<OverlayChange>();
        app.add_message::<GotoChange>();
        app.add_message::<SpawnItem>();
        app.add_message::<SpawnNpc>();
        app.add_message::<DialogClose>();
        app.add_message::<DialogLine>();
        app.add_message::<InventoryGain>();
        app.add_message::<JournalChange>();
        app.add_message::<ScreenMessage>();
        app.add_systems(
            Update,
            (
                send_script_changes,
                log_inventory_gains,
                log_journal_changes,
                forward_screen_messages,
                apply_routine_changes,
                apply_look_at_changes,
                apply_weapon_mode_changes,
                apply_equip_item_changes,
                apply_health_changes,
                apply_overlay_changes,
                seed_npc_components_on_add,
            )
                .chain()
                .in_set(ScriptChangesApplied)
                .after(advance_game_time)
                .after(InstanceIndexUpdated),
        );
    }
}

/// Marks systems that have applied script produced changes to the ECS. Systems
/// reacting to those changes run after this set.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ScriptChangesApplied;

/// Drains all the changes the script recorded and forwards them as messages.
fn send_script_changes(
    mut routine: MessageWriter<RoutineEntryChange>,
    mut look_at: MessageWriter<NpcLookAtChange>,
    mut weapon_mode: MessageWriter<WeaponModeChange>,
    mut equip_item: MessageWriter<EquipItemChange>,
    mut health: MessageWriter<HealthChange>,
    mut align_to_wp: MessageWriter<AlignToWpChange>,
    mut overlay: MessageWriter<OverlayChange>,
    mut goto: MessageWriter<GotoChange>,
    mut spawn_item: MessageWriter<SpawnItem>,
    mut spawn_npc: MessageWriter<SpawnNpc>,
    mut dialog_close: MessageWriter<DialogClose>,
    mut dialog_line: MessageWriter<DialogLine>,
    mut inventory_gain: MessageWriter<InventoryGain>,
    mut journal_change: MessageWriter<JournalChange>,
    gothic_script: Res<ScriptResource>,
) {
    let mut script = gothic_script.lock();
    for change in script.drain_routine_changes() {
        routine.write(change);
    }
    for change in script.drain_look_at_changes() {
        look_at.write(change);
    }
    for change in script.drain_weapon_mode_changes() {
        weapon_mode.write(change);
    }
    for change in script.drain_equip_item_changes() {
        equip_item.write(change);
    }
    for change in script.drain_health_changes() {
        health.write(change);
    }
    for change in script.drain_align_to_wp_changes() {
        align_to_wp.write(change);
    }
    for change in script.drain_overlay_changes() {
        overlay.write(change);
    }
    for change in script.drain_goto_changes() {
        goto.write(change);
    }
    for change in script.drain_spawn_items() {
        spawn_item.write(change);
    }
    for change in script.drain_spawn_npcs() {
        spawn_npc.write(change);
    }
    if script.take_close_dialog() {
        dialog_close.write(DialogClose);
    }
    for line in script.drain_dialog_lines() {
        dialog_line.write(line);
    }
    for gain in script.drain_inventory_gains() {
        inventory_gain.write(gain);
    }
    for change in script.drain_journal_changes() {
        journal_change.write(change);
    }
    // `Npc_ExchangeRoutine` cannot run its `Rtn_*` function from inside the
    // host import (wasmtime traps on re-entrancy), so it is resolved here: the
    // function refills the npc's routine entries and the produced routine entry
    // changes are forwarded in the same frame.
    for exchange in script.drain_routine_exchanges() {
        script.exchange_routine(exchange.instance_id, &exchange.routine_name);
    }
    for change in script.drain_routine_changes() {
        routine.write(change);
    }
}

/// Forwards the on-screen messages the script requested as ECS messages.
fn forward_screen_messages(
    mut screen_message: MessageWriter<ScreenMessage>,
    gothic_script: Res<ScriptResource>,
) {
    let mut script = gothic_script.lock();
    for message in script.drain_screen_messages() {
        screen_message.write(message);
    }
}

/// Shows the items the player's inventory gained through the script in the
/// bottom-left game log.
fn log_inventory_gains(
    mut gains: MessageReader<InventoryGain>,
    mut game_log: MessageWriter<GameLogEntry>,
    gothic_script: Res<ScriptResource>,
) {
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

/// Shows the journal (quest log) changes the script made in the bottom-left
/// game log.
fn log_journal_changes(
    mut changes: MessageReader<JournalChange>,
    mut game_log: MessageWriter<GameLogEntry>,
) {
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

/// Applies routine entry changes to the entities carrying the affected instance
/// ids. An entity gets the routine entry active at the current minute, or loses
/// the component when its instance has no routine entries.
fn apply_routine_changes(
    mut commands: Commands,
    mut changes: MessageReader<RoutineEntryChange>,
    index: Res<InstanceIndex>,
    game_time: Res<GameTime>,
    gothic_script: Res<ScriptResource>,
) {
    if changes.is_empty() {
        return;
    }

    let minutes = game_time.minutes_of_day();
    let gothic_script = gothic_script.lock();
    let instances = gothic_script.npc_instances();

    for change in changes.read() {
        let active = instances
            .get(&change.instance_id)
            .and_then(|npc| npc.get_routine_entry(minutes));
        for entity in index.npcs(change.instance_id) {
            set_routine_entry(&mut commands, *entity, active);
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

/// Applies look-at target changes to the entities carrying the affected instance
/// ids. An entity gets the target its instance currently looks at, or loses the
/// component when it looks at nothing.
fn apply_look_at_changes(
    mut commands: Commands,
    mut changes: MessageReader<NpcLookAtChange>,
    index: Res<InstanceIndex>,
    gothic_script: Res<ScriptResource>,
) {
    if changes.is_empty() {
        return;
    }

    let gothic_script = gothic_script.lock();
    let instances = gothic_script.npc_instances();

    for change in changes.read() {
        let target = instances
            .get(&change.instance_id)
            .and_then(|npc| npc.look_at)
            .and_then(|id| index.npcs(id).first().copied());
        for entity in index.npcs(change.instance_id) {
            set_look_at(&mut commands, *entity, target);
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

/// Applies weapon mode changes to the entities carrying the affected instance
/// ids. An entity gets the mode its instance currently has, or loses the
/// component when it has none.
fn apply_weapon_mode_changes(
    mut commands: Commands,
    mut changes: MessageReader<WeaponModeChange>,
    index: Res<InstanceIndex>,
    gothic_script: Res<ScriptResource>,
) {
    if changes.is_empty() {
        return;
    }

    let gothic_script = gothic_script.lock();
    let instances = gothic_script.npc_instances();

    for change in changes.read() {
        let mode = instances
            .get(&change.instance_id)
            .and_then(|npc| npc.weapon_mode);
        for entity in index.npcs(change.instance_id) {
            set_weapon_mode(&mut commands, *entity, mode);
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

/// Applies equipped item changes to the entities carrying the affected instance
/// ids. An entity gets the melee/ranged weapons its instance currently has
/// equipped, or loses the component when it has none.
fn apply_equip_item_changes(
    mut commands: Commands,
    mut changes: MessageReader<EquipItemChange>,
    index: Res<InstanceIndex>,
    gothic_script: Res<ScriptResource>,
) {
    if changes.is_empty() {
        return;
    }

    let gothic_script = gothic_script.lock();
    let instances = gothic_script.npc_instances();

    for change in changes.read() {
        let equipped = instances
            .get(&change.instance_id)
            .map(|npc| NpcEquippedWeapon {
                melee: npc.equipped_melee,
                ranged: npc.equipped_ranged,
            });
        for entity in index.npcs(change.instance_id) {
            set_equipped_weapon(&mut commands, *entity, equipped.clone());
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

/// Applies hitpoints changes to the entities carrying the affected instance
/// ids. Seeds the instance hitpoints from the npc instance data on first
/// access.
fn apply_health_changes(
    mut commands: Commands,
    mut changes: MessageReader<HealthChange>,
    index: Res<InstanceIndex>,
    gothic_script: Res<ScriptResource>,
) {
    if changes.is_empty() {
        return;
    }

    let mut gothic_script = gothic_script.lock();
    for change in changes.read() {
        let Some((hitpoints, hitpoints_max)) = gothic_script.hitpoints(change.instance_id) else {
            continue;
        };
        for entity in index.npcs(change.instance_id) {
            // The npc may have been despawned earlier in this frame, skip it.
            commands
                .entity(*entity)
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
/// produced before it existed, so it catches up from the current script state.
fn seed_npc_components_on_add(
    mut commands: Commands,
    added_npcs: Query<(Entity, &GameNpc), Added<GameNpc>>,
    index: Res<InstanceIndex>,
    game_time: Res<GameTime>,
    gothic_script: Res<ScriptResource>,
) {
    if added_npcs.is_empty() {
        return;
    }

    let minutes = game_time.minutes_of_day();
    let mut gothic_script = gothic_script.lock();
    // Seed the hitpoints from the npc instance data first, this needs `&mut`
    let mut healths = HashMap::new();
    for (_, npc) in &added_npcs {
        if let Some((hitpoints, hitpoints_max)) = gothic_script.hitpoints(npc.npc_instance_id) {
            healths.insert(
                npc.npc_instance_id,
                NpcHealth::new(hitpoints, hitpoints_max),
            );
        } else {
            warn!(
                "No hitpoints data for npc({}) on spawn",
                npc.npc_instance_id.value()
            );
        }
    }
    let instances = gothic_script.npc_instances();

    for (entity, npc) in &added_npcs {
        let Some(instance) = instances.get(&npc.npc_instance_id) else {
            continue;
        };
        if let Some(health) = healths.get(&npc.npc_instance_id) {
            commands.entity(entity).insert(*health);
        }
        set_routine_entry(&mut commands, entity, instance.get_routine_entry(minutes));
        let target = instance
            .look_at
            .and_then(|id| index.npcs(id).first().copied());
        set_look_at(&mut commands, entity, target);
        set_weapon_mode(&mut commands, entity, instance.weapon_mode);
        let equipped = (instance.equipped_melee.is_some() || instance.equipped_ranged.is_some())
            .then_some(NpcEquippedWeapon {
                melee: instance.equipped_melee,
                ranged: instance.equipped_ranged,
            });
        set_equipped_weapon(&mut commands, entity, equipped);
    }
}
