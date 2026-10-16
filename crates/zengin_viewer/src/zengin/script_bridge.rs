//! Bridges changes produced inside the script vm to the ECS. The script appends
//! change records to its state, `send_script_changes` drains them into Bevy
//! messages and the matching `apply_*` systems turn them into component changes
//! on the affected entities.

use bevy::prelude::*;

use crate::gameplay::game_time::{GameTime, advance_game_time};
use crate::zengin::GameNpc;
use crate::zengin::instance_index::{InstanceIndex, InstanceIndexUpdated};
use crate::zengin::npc_routines::{
    NpcEquippedWeapon, NpcLookAt, NpcOverlay, NpcWeaponMode, overlay_scheme,
};
use crate::zengin::script::component::ScriptResource;
use crate::zengin::script::state::{
    AlignToWpChange, DialogClose, DialogLine, DialogRoot, EquipItemChange, GotoChange,
    NpcLookAtChange, OverlayChange, OverlayRequest, RoutineEntry, RoutineEntryChange, SpawnItem,
    SpawnNpc, WeaponMode, WeaponModeChange,
};
use crate::zengin::script::symbol_id::InstanceId;

/// Marks systems that have applied script produced changes to the ECS. Systems
/// reacting to those changes run after this set.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ScriptChangesApplied;

pub struct ScriptBridgePlugin;

impl Plugin for ScriptBridgePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<RoutineEntryChange>();
        app.add_message::<NpcLookAtChange>();
        app.add_message::<WeaponModeChange>();
        app.add_message::<EquipItemChange>();
        app.add_message::<AlignToWpChange>();
        app.add_message::<OverlayChange>();
        app.add_message::<GotoChange>();
        app.add_message::<SpawnItem>();
        app.add_message::<SpawnNpc>();
        app.add_message::<DialogClose>();
        app.add_message::<DialogLine>();
        app.add_message::<DialogRoot>();
        app.add_systems(
            Update,
            (
                send_script_changes,
                apply_routine_changes,
                apply_look_at_changes,
                apply_weapon_mode_changes,
                apply_equip_item_changes,
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

/// Drains all the changes the script recorded and forwards them as messages.
fn send_script_changes(
    mut routine: MessageWriter<RoutineEntryChange>,
    mut look_at: MessageWriter<NpcLookAtChange>,
    mut weapon_mode: MessageWriter<WeaponModeChange>,
    mut equip_item: MessageWriter<EquipItemChange>,
    mut align_to_wp: MessageWriter<AlignToWpChange>,
    mut overlay: MessageWriter<OverlayChange>,
    mut goto: MessageWriter<GotoChange>,
    mut spawn_item: MessageWriter<SpawnItem>,
    mut spawn_npc: MessageWriter<SpawnNpc>,
    mut dialog_close: MessageWriter<DialogClose>,
    mut dialog_line: MessageWriter<DialogLine>,
    mut sub_dialog_choice: MessageWriter<DialogRoot>,
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
    for choice in script.drain_sub_dialog_choices() {
        sub_dialog_choice.write(choice);
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
/// the instance has no routine entry active.
fn set_routine_entry(commands: &mut Commands, entity: Entity, active: Option<&RoutineEntry>) {
    match active {
        Some(active) => {
            commands.entity(entity).insert(active.clone());
        }
        None => {
            commands.entity(entity).remove::<RoutineEntry>();
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
            commands.entity(entity).insert(NpcLookAt(target));
        }
        None => {
            commands.entity(entity).remove::<NpcLookAt>();
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
            commands.entity(entity).insert(NpcWeaponMode(mode));
        }
        None => {
            commands.entity(entity).remove::<NpcWeaponMode>();
        }
    }
}

/// Applies equipped item changes to the entities carrying the affected instance
/// ids. An entity gets the item its instance currently has equipped, or loses
/// the component when it has none.
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
        let item = instances
            .get(&change.instance_id)
            .and_then(|npc| npc.equipped_weapon);
        for entity in index.npcs(change.instance_id) {
            set_equipped_weapon(&mut commands, *entity, item);
        }
    }
}

/// Inserts the equipped item on the entity, or removes the component when the
/// npc has no item equipped.
fn set_equipped_weapon(commands: &mut Commands, entity: Entity, item: Option<InstanceId>) {
    match item {
        Some(item) => {
            commands.entity(entity).insert(NpcEquippedWeapon(item));
        }
        None => {
            commands.entity(entity).remove::<NpcEquippedWeapon>();
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
            commands.entity(*entity).insert(NpcOverlay(scheme.clone()));
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
    let gothic_script = gothic_script.lock();
    let instances = gothic_script.npc_instances();

    for (entity, npc) in &added_npcs {
        let Some(instance) = instances.get(&npc.npc_instance_id) else {
            continue;
        };
        set_routine_entry(&mut commands, entity, instance.get_routine_entry(minutes));
        let target = instance
            .look_at
            .and_then(|id| index.npcs(id).first().copied());
        set_look_at(&mut commands, entity, target);
        set_weapon_mode(&mut commands, entity, instance.weapon_mode);
        set_equipped_weapon(&mut commands, entity, instance.equipped_weapon);
    }
}
