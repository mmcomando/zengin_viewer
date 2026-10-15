//! Bridges changes produced inside the script vm to the ECS. The script appends
//! change records to its state, the `send_*` systems drain them into Bevy
//! messages and the matching `apply_*` systems turn them into component changes
//! on the affected entities.

use bevy::prelude::*;

use crate::gameplay::game_time::{GameTime, advance_game_time};
use crate::zengin::GameNpc;
use crate::zengin::instance_index::InstanceIndex;
use crate::zengin::script::component::ScriptResource;
use crate::zengin::script::state::{RoutineEntry, RoutineEntryChange};

/// Marks systems that have applied script produced changes to the ECS. Systems
/// reacting to those changes run after this set.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ScriptChangesApplied;

pub struct ScriptBridgePlugin;

impl Plugin for ScriptBridgePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<RoutineEntryChange>();
        app.add_systems(
            Update,
            (send_routine_changes, apply_routine_changes)
                .chain()
                .in_set(ScriptChangesApplied)
                .after(advance_game_time),
        );
    }
}

/// Drains the routine entry changes the script recorded and forwards them as messages.
fn send_routine_changes(
    mut changes: MessageWriter<RoutineEntryChange>,
    gothic_script: Res<ScriptResource>,
) {
    let drained = gothic_script.lock().drain_routine_changes();
    for change in drained {
        changes.write(change);
    }
}

/// Applies routine entry changes to the entities carrying the affected instance
/// ids and seeds the component on newly spawned npc entities. An entity gets the
/// routine entry active at the current minute, or loses the component when its
/// instance has no routine entries.
fn apply_routine_changes(
    mut commands: Commands,
    mut changes: MessageReader<RoutineEntryChange>,
    index: Res<InstanceIndex>,
    game_time: Res<GameTime>,
    gothic_script: Res<ScriptResource>,
    added_npcs: Query<(Entity, &GameNpc), Added<GameNpc>>,
) {
    if changes.is_empty() && added_npcs.is_empty() {
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
    for (entity, npc) in &added_npcs {
        let active = instances
            .get(&npc.npc_instance_id)
            .and_then(|npc| npc.get_routine_entry(minutes));
        set_routine_entry(&mut commands, entity, active);
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
