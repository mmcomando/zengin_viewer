use bevy::prelude::*;

use crate::AnimationController;
use crate::instance_index::InstanceIndex;
use crate::npc_actions::action::ExecutingActionMarker;
use crate::npc_actions::attack::AttackAction;
use crate::npc_actions::goto::GoToWaypointAction;
use crate::npc_actions::goto_npc::GoToNpcAction;
use crate::npc_actions::item_state::UseItemStateAction;
use crate::npc_actions::queue::NpcActionQueue;
use crate::npc_actions::use_mob::UseMobAction;
use crate::npc_routines::{NpcRoutineState, RoutineExecutor};
use crate::script::component::ScriptResource;
use crate::script::state::{
    AlignToWpChange, AttackChange, ClearAiQueueChange, GotoChange, GotoFpChange, GotoNpcChange,
    StandUpChange, StartStateChange, UseItemStateChange, UseMobChange, WaitChange,
};
use crate::script::string_value::StringCanon;
use crate::script::symbol_id::{InstanceId, StringId};
use dev_util::perf_debug;

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

/// Queues a goto for every entity of `instance_id`, used by the replay command
/// that names a waypoint directly (there is no script string id to resolve).
/// Cancels a goto that is already executing, like [`enqueue_goto_actions`].
pub fn enqueue_npc_goto(world: &mut World, instance_id: InstanceId, waypoint: StringCanon) {
    let Some(index) = world.get_resource::<InstanceIndex>() else {
        warn!("npc goto: instance index is not registered");
        return;
    };
    let entities = index.npcs(instance_id).to_vec();
    if entities.is_empty() {
        warn!("npc goto: npc({instance_id}) has no entities");
        return;
    }

    let mut queued = false;
    for entity in entities {
        let executing = world.get::<GoToWaypointAction>(entity).is_some();
        let pushed = {
            let Some(mut queue) = world.get_mut::<NpcActionQueue>(entity) else {
                continue;
            };
            queue.push_goto(waypoint.clone())
        };
        queued |= pushed;
        if executing {
            world.entity_mut(entity).remove::<ExecutingActionMarker>();
        }
    }
    if !queued {
        warn!(
            "npc goto({waypoint}) was not queued, npc({instance_id}) has no action queue or it is full"
        );
    }
}

/// Turns `AI_GoToWP` requests into queued actions. The waypoint name is resolved
/// through the script vm here because a host import cannot safely call into the
/// guest while it is running.
#[allow(clippy::cognitive_complexity)]
pub fn enqueue_goto_actions(
    mut commands: Commands,
    mut changes: MessageReader<GotoChange>,
    index: Res<InstanceIndex>,
    gothic_script: Res<ScriptResource>,
    executing: Query<&GoToWaypointAction>,
    mut action_queues: Query<&mut NpcActionQueue>,
) {
    let _perf = perf_debug!();
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
pub fn enqueue_goto_fp_actions(
    mut commands: Commands,
    mut changes: MessageReader<GotoFpChange>,
    index: Res<InstanceIndex>,
    executing: Query<&GoToWaypointAction>,
    mut action_queues: Query<&mut NpcActionQueue>,
) {
    let _perf = perf_debug!();
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

/// Turns `AI_GotoNpc` requests into queued actions on the target entity. The
/// target instance may have several entities (duplicated spawns), the closest
/// one is followed.
pub fn enqueue_goto_npc_actions(
    mut changes: MessageReader<GotoNpcChange>,
    index: Res<InstanceIndex>,
    executing: Query<&GoToNpcAction>,
    transforms: Query<&Transform>,
    mut action_queues: Query<&mut NpcActionQueue>,
) {
    let _perf = perf_debug!();
    for change in changes.read() {
        enqueue_goto_npc_change(change, &index, &executing, &transforms, &mut action_queues);
    }
}

#[allow(clippy::cognitive_complexity)]
fn enqueue_goto_npc_change(
    change: &GotoNpcChange,
    index: &InstanceIndex,
    executing: &Query<&GoToNpcAction>,
    transforms: &Query<&Transform>,
    action_queues: &mut Query<&mut NpcActionQueue>,
) {
    if change.instance_id == change.target {
        warn!(
            "AI_GotoNpc npc({}) ignored: it targets itself",
            change.instance_id
        );
        return;
    }
    let target_entities = index.npcs(change.target);
    if target_entities.is_empty() {
        warn!(
            "AI_GotoNpc npc({}) ignored: target({}) has no entity",
            change.instance_id, change.target
        );
        return;
    }
    let entities = index.npcs(change.instance_id);
    if entities.is_empty() {
        warn!(
            "AI_GotoNpc npc({}) ignored: instance has no entity",
            change.instance_id
        );
        return;
    }
    for &entity in entities {
        let Ok(mut queue) = action_queues.get_mut(entity) else {
            warn!(
                "AI_GotoNpc npc({}) on entity({entity}) ignored: no action queue",
                change.instance_id
            );
            continue;
        };
        enqueue_goto_npc_for_entity(
            *change,
            entity,
            target_entities,
            executing,
            transforms,
            &mut queue,
        );
    }
}

/// Picks the closest target entity and queues it unless it is already followed.
fn enqueue_goto_npc_for_entity(
    change: GotoNpcChange,
    entity: Entity,
    target_entities: &[Entity],
    executing: &Query<&GoToNpcAction>,
    transforms: &Query<&Transform>,
    queue: &mut NpcActionQueue,
) {
    let target = nearest_target(entity, target_entities, transforms).unwrap_or(target_entities[0]);
    if executing
        .get(entity)
        .is_ok_and(|action| action.target == target)
    {
        return;
    }
    if !queue.push_goto_npc(target) {
        warn!(
            "AI_GotoNpc npc({}) target({}) not queued: action queue is full",
            change.instance_id, change.target
        );
    }
}

/// Turns `AI_StartState` requests into queued start actions. The action waits
/// behind the running one and only the state-owning entity gets it.
pub fn enqueue_start_state_actions(
    mut changes: MessageReader<StartStateChange>,
    index: Res<InstanceIndex>,
    executors: Query<(), With<RoutineExecutor>>,
    controllers: Query<(), With<AnimationController>>,
    mut action_queues: Query<&mut NpcActionQueue>,
) {
    let _perf = perf_debug!();
    for change in changes.read() {
        let entities = index.npcs(change.instance_id);
        // The state functions run per script instance; duplicated spawns share
        // one instance, so only one entity owns the executor.
        let owner = entities
            .iter()
            .copied()
            .find(|&entity| executors.contains(entity))
            .or_else(|| {
                entities
                    .iter()
                    .copied()
                    .find(|&entity| controllers.contains(entity))
            });
        let Some(entity) = owner else {
            continue;
        };
        let Ok(mut queue) = action_queues.get_mut(entity) else {
            continue;
        };
        if !queue.push_start_state(change.state, change.other) {
            warn!(
                "AI_StartState npc({}) not queued: action queue is full",
                change.instance_id
            );
        }
    }
}

/// Turns `AI_WaitMS` requests into queued wait actions. The host drops
/// non-positive durations, so no second guard is needed here.
pub fn enqueue_wait_actions(
    mut changes: MessageReader<WaitChange>,
    index: Res<InstanceIndex>,
    mut action_queues: Query<&mut NpcActionQueue>,
) {
    let _perf = perf_debug!();
    for change in changes.read() {
        let entities = index.npcs(change.instance_id);
        if entities.is_empty() {
            warn!(
                "AI_WaitMS npc({}) ignored: instance has no entity",
                change.instance_id
            );
            continue;
        }
        let remaining = change.milliseconds as f32 / 1000.0;
        for &entity in entities {
            let Ok(mut queue) = action_queues.get_mut(entity) else {
                continue;
            };
            if !queue.push_wait(remaining) {
                warn!(
                    "AI_WaitMS npc({}) not queued: action queue is full",
                    change.instance_id
                );
            }
        }
    }
}

/// Turns `AI_StandUp`/`AI_StandUpQuick` requests into queued actions.
pub fn enqueue_stand_up_actions(
    mut changes: MessageReader<StandUpChange>,
    index: Res<InstanceIndex>,
    mut action_queues: Query<&mut NpcActionQueue>,
) {
    let _perf = perf_debug!();
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
pub fn enqueue_align_actions(
    mut commands: Commands,
    mut changes: MessageReader<AlignToWpChange>,
    index: Res<InstanceIndex>,
    executing: Query<&GoToWaypointAction>,
    mut npcs: Query<(&NpcRoutineState, &mut NpcActionQueue)>,
) {
    let _perf = perf_debug!();
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

/// Applies `Npc_ClearAIQueue` requests. The engine clears the queue and the
/// action the npc is currently running, which also stops a walk.
#[allow(clippy::type_complexity)]
pub fn clear_npc_actions(
    mut commands: Commands,
    mut changes: MessageReader<ClearAiQueueChange>,
    index: Res<InstanceIndex>,
    mut action_queues: Query<(&mut NpcActionQueue, Has<ExecutingActionMarker>)>,
) {
    let _perf = perf_debug!();
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
            let Ok((mut queue, executing)) = action_queues.get_mut(entity) else {
                continue;
            };
            queue.clear();
            // The marker observer removes the action components.
            if executing {
                cancel_executing_action(&mut commands, entity);
            }
            debug!(
                "Npc_ClearAIQueue npc({}) cleared actions entity({entity})",
                change.instance_id
            );
        }
    }
}

fn cancel_executing_action(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).remove::<ExecutingActionMarker>();
}

/// Turns `AI_UseMob` requests into queued actions.
pub fn enqueue_use_mob_actions(
    mut changes: MessageReader<UseMobChange>,
    index: Res<InstanceIndex>,
    mut executing: Query<&mut UseMobAction>,
    mut action_queues: Query<&mut NpcActionQueue>,
) {
    let _perf = perf_debug!();
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
            if change.state <= 0 {
                continue;
            }
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
pub fn enqueue_use_item_state_actions(
    mut changes: MessageReader<UseItemStateChange>,
    index: Res<InstanceIndex>,
    executing: Query<&UseItemStateAction>,
    mut action_queues: Query<&mut NpcActionQueue>,
) {
    let _perf = perf_debug!();
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
/// target. An attack already running or queued on the same target is a no-op, so
/// a state start and its `zs_attack_loop` may both request it.
#[allow(clippy::cognitive_complexity)]
pub fn enqueue_attack_actions(
    mut changes: MessageReader<AttackChange>,
    index: Res<InstanceIndex>,
    executing: Query<&AttackAction>,
    transforms: Query<&Transform>,
    mut action_queues: Query<&mut NpcActionQueue>,
) {
    let _perf = perf_debug!();
    for change in changes.read() {
        if change.instance_id == change.target {
            warn!(
                "AI_Attack npc({}) ignored: it targets itself",
                change.instance_id
            );
            continue;
        }
        let target_entities = index.npcs(change.target);
        if target_entities.is_empty() {
            warn!(
                "AI_Attack npc({}) ignored: target({}) has no entity",
                change.instance_id, change.target
            );
            continue;
        }
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
            // Several entities can share one script instance (`Wld_InsertNpc`
            // spawns the same wolf instance many times), so attack the closest
            // one instead of the first one the index returns.
            let target =
                nearest_target(entity, target_entities, &transforms).unwrap_or(target_entities[0]);
            if executing
                .get(entity)
                .is_ok_and(|attack| attack.target == target)
            {
                continue;
            }
            if queue.has_attack(target) {
                continue;
            }
            queue.push_attack(target);
            info!(
                "AI_Attack npc({}) queued against target({}) entity({entity})",
                change.instance_id, change.target
            );
        }
    }
}

fn nearest_target(
    from: Entity,
    targets: &[Entity],
    transforms: &Query<&Transform>,
) -> Option<Entity> {
    let from = transforms.get(from).ok()?.translation;
    targets.iter().copied().min_by(|left, right| {
        let left = transforms.get(*left).map_or(f32::MAX, |transform| {
            (transform.translation - from).length_squared()
        });
        let right = transforms.get(*right).map_or(f32::MAX, |transform| {
            (transform.translation - from).length_squared()
        });
        left.total_cmp(&right)
    })
}
