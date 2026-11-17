use bevy::prelude::*;

use crate::zengin::instance_index::InstanceIndex;
use crate::zengin::npc_actions::action::ExecutingActionMarker;
use crate::zengin::npc_actions::goto::GoToWaypointAction;
use crate::zengin::npc_actions::item_state::UseItemStateAction;
use crate::zengin::npc_actions::queue::NpcActionQueue;
use crate::zengin::npc_actions::use_mob::UseMobAction;
use crate::zengin::npc_routines::NpcRoutineState;
use crate::zengin::script::component::ScriptResource;
use crate::zengin::script::state::{
    AlignToWpChange, AttackChange, ClearAiQueueChange, GotoChange, GotoFpChange, StandUpChange,
    UseItemStateChange, UseMobChange,
};
use crate::zengin::script::string_value::StringCanon;
use crate::zengin::script::symbol_id::StringId;

/// Queues a goto and cancels a goto that is already executing.
fn queue_goto(
    commands: &mut Commands,
    entity: Entity,
    waypoint: StringCanon,
    queue: &mut NpcActionQueue,
    executing: &Query<&GoToWaypointAction>,
) -> bool {
    if !queue.push_goto(waypoint) {
        return false;
    }
    if executing.get(entity).is_ok() {
        commands.entity(entity).remove::<ExecutingActionMarker>();
    }
    true
}

/// Turns `AI_GoToWP` requests into queued actions. The waypoint name is resolved
/// through the script vm here because a host import cannot safely call into the
/// guest while it is running.
#[allow(clippy::cognitive_complexity)]
pub(crate) fn enqueue_goto_actions(
    mut commands: Commands,
    mut changes: MessageReader<GotoChange>,
    index: Res<InstanceIndex>,
    gothic_script: Res<ScriptResource>,
    executing: Query<&GoToWaypointAction>,
    mut action_queues: Query<&mut NpcActionQueue>,
) {
    if changes.is_empty() {
        return;
    }

    let mut script = gothic_script.lock();
    for change in changes.read() {
        // String id 0 is unset; scripts pass it when there is no waypoint.
        if change.waypoint == 0 {
            continue;
        }
        let Some(string_id) = StringId::new(change.waypoint) else {
            warn!(
                "AI_GoToWP with invalid waypoint string id({}), npc({})",
                change.waypoint, change.instance_id
            );
            continue;
        };
        let Some(waypoint) = script.get_canon_string(string_id) else {
            warn!(
                "AI_GoToWP with unknown waypoint string id({}), npc({})",
                change.waypoint, change.instance_id
            );
            continue;
        };

        let mut queued = false;
        for &entity in index.npcs(change.instance_id) {
            let Ok(mut queue) = action_queues.get_mut(entity) else {
                continue;
            };
            queued |= queue_goto(
                &mut commands,
                entity,
                waypoint.clone(),
                &mut queue,
                &executing,
            );
        }
        if !queued {
            warn!(
                "AI_GoToWP waypoint({}) was not queued, npc({}) has no action queue or it is full",
                waypoint, change.instance_id
            );
        }
    }
}

/// Turns `AI_GotoFP`/`AI_GotoNextFP` requests into queued actions. The freepoint
/// is resolved by the script host, the name is a way-net point the action queue
/// can walk to.
pub(crate) fn enqueue_goto_fp_actions(
    mut commands: Commands,
    mut changes: MessageReader<GotoFpChange>,
    index: Res<InstanceIndex>,
    executing: Query<&GoToWaypointAction>,
    mut action_queues: Query<&mut NpcActionQueue>,
) {
    for change in changes.read() {
        let entities = index.npcs(change.instance_id);
        if entities.is_empty() {
            warn!(
                "AI_GotoFP freepoint({}) ignored: npc({}) has no entity",
                change.point_name, change.instance_id
            );
            continue;
        }
        for &entity in entities {
            let Ok(mut queue) = action_queues.get_mut(entity) else {
                continue;
            };
            if !queue_goto(
                &mut commands,
                entity,
                change.point_name.clone(),
                &mut queue,
                &executing,
            ) {
                warn!(
                    "AI_GotoFP freepoint({}) was not queued, npc({}) action queue is full",
                    change.point_name, change.instance_id
                );
            }
        }
    }
}

/// Turns `AI_StandUp`/`AI_StandUpQuick` requests into queued actions.
pub(crate) fn enqueue_stand_up_actions(
    mut changes: MessageReader<StandUpChange>,
    index: Res<InstanceIndex>,
    mut action_queues: Query<&mut NpcActionQueue>,
) {
    for change in changes.read() {
        let entities = index.npcs(change.instance_id);
        if entities.is_empty() {
            warn!(
                "AI_StandUp npc({}) ignored: instance has no entity",
                change.instance_id
            );
            continue;
        }
        for &entity in entities {
            let Ok(mut queue) = action_queues.get_mut(entity) else {
                continue;
            };
            if !queue.push_stand_up() {
                warn!(
                    "AI_StandUp npc({}) not queued: action queue is full",
                    change.instance_id
                );
            }
        }
    }
}

/// Turns `AI_AlignToWP` requests into queued actions targeting the npc's
/// assigned routine waypoint. The action completes without walking when the npc
/// already stands on the waypoint.
pub(crate) fn enqueue_align_actions(
    mut commands: Commands,
    mut changes: MessageReader<AlignToWpChange>,
    index: Res<InstanceIndex>,
    executing: Query<&GoToWaypointAction>,
    mut npcs: Query<(&NpcRoutineState, &mut NpcActionQueue)>,
) {
    for change in changes.read() {
        for &entity in index.npcs(change.instance_id) {
            let Ok((routine, mut queue)) = npcs.get_mut(entity) else {
                continue;
            };
            if !queue_goto(
                &mut commands,
                entity,
                routine.assigned_waypoint().clone(),
                &mut queue,
                &executing,
            ) {
                warn!(
                    "AI_AlignToWP waypoint({}) was not queued, npc({}) action queue is full",
                    routine.assigned_waypoint(),
                    change.instance_id
                );
            }
        }
    }
}

/// Applies `Npc_ClearAIQueue` requests, dropping all queued actions.
pub(crate) fn clear_npc_actions(
    mut changes: MessageReader<ClearAiQueueChange>,
    index: Res<InstanceIndex>,
    mut action_queues: Query<&mut NpcActionQueue>,
) {
    for change in changes.read() {
        let entities = index.npcs(change.instance_id);
        if entities.is_empty() {
            warn!(
                "Npc_ClearAIQueue npc({}) ignored: instance has no entity",
                change.instance_id
            );
            continue;
        }
        for &entity in entities {
            let Ok(mut queue) = action_queues.get_mut(entity) else {
                continue;
            };
            queue.clear();
            debug!(
                "Npc_ClearAIQueue npc({}) cleared actions entity({entity})",
                change.instance_id
            );
        }
    }
}

/// Turns `AI_UseMob` requests into queued actions.
pub(crate) fn enqueue_use_mob_actions(
    mut changes: MessageReader<UseMobChange>,
    index: Res<InstanceIndex>,
    mut executing: Query<&mut UseMobAction>,
    mut action_queues: Query<&mut NpcActionQueue>,
) {
    for change in changes.read() {
        let entities = index.npcs(change.instance_id);
        if entities.is_empty() {
            warn!(
                "AI_UseMob npc({}) ignored: instance has no entity",
                change.instance_id
            );
            continue;
        }
        for &entity in entities {
            let Ok(mut queue) = action_queues.get_mut(entity) else {
                continue;
            };
            if let Ok(mut action) = executing.get_mut(entity)
                && action.scheme == change.mob_name
            {
                action.state = change.state;
                continue;
            }
            if !queue.push_use_mob(change.mob_name.clone(), change.state) {
                warn!(
                    "AI_UseMob npc({}) scheme({}) not queued: action queue is full",
                    change.instance_id, change.mob_name
                );
            }
        }
    }
}

/// Turns `AI_UseItemToState` requests into queued actions.
pub(crate) fn enqueue_use_item_state_actions(
    mut changes: MessageReader<UseItemStateChange>,
    index: Res<InstanceIndex>,
    executing: Query<&UseItemStateAction>,
    mut action_queues: Query<&mut NpcActionQueue>,
) {
    for change in changes.read() {
        let entities = index.npcs(change.instance_id);
        if entities.is_empty() {
            warn!(
                "AI_UseItemToState npc({}) item({}) ignored: instance has no entity",
                change.instance_id,
                change.item.value()
            );
            continue;
        }
        for &entity in entities {
            let Ok(mut queue) = action_queues.get_mut(entity) else {
                continue;
            };
            if let Ok(action) = executing.get(entity)
                && action.item == change.item
                && action.state == change.state
            {
                continue;
            }
            if !queue.push_use_item_state(change.item, change.scheme.clone(), change.state) {
                warn!(
                    "AI_UseItemToState npc({}) item({}) not queued: action queue is full",
                    change.instance_id,
                    change.item.value()
                );
            }
        }
    }
}

/// Turns `AI_Attack` requests into queued actions on their `Npc_SetTarget`
/// target.
#[allow(clippy::cognitive_complexity)]
pub(crate) fn enqueue_attack_actions(
    mut changes: MessageReader<AttackChange>,
    index: Res<InstanceIndex>,
    mut action_queues: Query<&mut NpcActionQueue>,
) {
    for change in changes.read() {
        if change.instance_id == change.target {
            warn!(
                "AI_Attack npc({}) ignored: it targets itself",
                change.instance_id
            );
            continue;
        }
        let Some(&target) = index.npcs(change.target).first() else {
            warn!(
                "AI_Attack npc({}) ignored: target({}) has no entity",
                change.instance_id, change.target
            );
            continue;
        };
        let entities = index.npcs(change.instance_id);
        if entities.is_empty() {
            warn!(
                "AI_Attack npc({}) ignored: instance has no entity",
                change.instance_id
            );
            continue;
        }
        for &entity in entities {
            let Ok(mut queue) = action_queues.get_mut(entity) else {
                warn!(
                    "AI_Attack npc({}) on entity({entity}) ignored: no action queue",
                    change.instance_id
                );
                continue;
            };
            queue.push_attack(target);
            info!(
                "AI_Attack npc({}) queued against target({}) entity({entity})",
                change.instance_id, change.target
            );
        }
    }
}
