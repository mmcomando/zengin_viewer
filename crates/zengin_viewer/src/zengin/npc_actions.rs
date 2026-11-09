//! FIFO of npc actions. The front action is executed every frame until it
//! reports completion; an empty queue leaves the npc standing still.

use std::collections::VecDeque;

use avian3d::prelude::SpatialQuery;
use bevy::prelude::*;

use crate::gameplay::objects::NpcModel;
use crate::graphics::{
    AnimationController, AutoAnimSequencer, DesiredAnimation, SetDesiredAnimation,
};
use crate::zengin::GameNpc;
use crate::zengin::instance_index::InstanceIndex;
use crate::zengin::npc_routines::{
    AnimationFiles, NPC_BODY_Y_OFFSET, NpcHealth, NpcOverlay, NpcPath, NpcRoutineState, WayPoints,
    queue_stand_animation,
};
use crate::zengin::resources::CachedAssetHandles;
use crate::zengin::script::component::ScriptResource;
use crate::zengin::script::state::{
    AlignToWpChange, AttackChange, ClearAiQueueChange, GotoChange, WeaponMode,
};
use crate::zengin::script::string_value::StringCanon;
use crate::zengin::script::symbol_id::{InstanceId, StringId};
use crate::zengin::waypoint_graph::{WaypointGraph, WaypointNodeIndex, nearest_waypoint_node};

/// Upper bound of queued actions, including the action at the front.
const MAX_QUEUED_ACTIONS: usize = 8;

/// Distance in Bevy meters within which an npc counts as standing on the target
/// waypoint and only has to align.
const ARRIVE_DISTANCE: f32 = 0.5;

/// Horizontal distance within which an attacking npc stops chasing and swings.
const ATTACK_RANGE: f32 = 1.0;

/// Horizontal distance beyond which an attack action is abandoned.
const ATTACK_MAX_DISTANCE: f32 = 100.0;

/// The action at the front is executed every frame until it is done.
#[derive(Component, Default)]
pub(crate) struct NpcActionQueue {
    actions: VecDeque<NpcAction>,
    /// Front action has already been started (its [`NpcPath`] was inserted).
    started: bool,
}

impl NpcActionQueue {
    /// Appends an action, `false` when the queue is full. A `GoToWaypoint`
    /// replaces the `GoToWaypoint` at the back instead of stacking on it.
    pub(crate) fn push(&mut self, action: NpcAction) -> bool {
        match action {
            NpcAction::GoToWaypoint { waypoint } => self.push_goto(waypoint),
            NpcAction::Attack { target } => self.push_attack(target),
        }
    }

    fn push_goto(&mut self, waypoint: StringCanon) -> bool {
        if let Some(NpcAction::GoToWaypoint { waypoint: last }) = self.actions.back_mut() {
            *last = waypoint;
            // The replaced action was the active front, its path must be replanned.
            if self.actions.len() == 1 {
                self.started = false;
            }
            return true;
        }
        if self.actions.len() >= MAX_QUEUED_ACTIONS {
            return false;
        }
        self.actions.push_back(NpcAction::GoToWaypoint { waypoint });
        true
    }

    /// An attack preempts everything else: it clears the queue and starts right
    /// away. A repeated request for the attack already at the front is a no-op
    /// so an ongoing swing is not restarted.
    fn push_attack(&mut self, target: Entity) -> bool {
        if let Some(NpcAction::Attack { target: current }) = self.actions.front()
            && *current == target
        {
            return true;
        }
        self.actions.clear();
        self.started = false;
        self.actions.push_back(NpcAction::Attack { target });
        true
    }

    fn front(&self) -> Option<&NpcAction> {
        self.actions.front()
    }

    fn pop_front(&mut self) {
        self.actions.pop_front();
        self.started = false;
    }

    /// Drops every queued action (`Npc_ClearAIQueue`).
    fn clear(&mut self) {
        self.actions.clear();
        self.started = false;
    }

    fn started(&self) -> bool {
        self.started
    }

    fn mark_started(&mut self) {
        self.started = true;
    }
}

#[derive(Clone, Debug)]
pub(crate) enum NpcAction {
    GoToWaypoint {
        waypoint: StringCanon,
    },
    /// Chase and melee the target entity until it dies, the target is gone or
    /// it is farther away than [`ATTACK_MAX_DISTANCE`].
    Attack {
        target: Entity,
    },
}

/// Turns `AI_GoToWP` requests into queued actions. The waypoint name is resolved
/// through the script vm here because a host import cannot safely call into the
/// guest while it is running.
#[allow(clippy::cognitive_complexity)]
pub(crate) fn enqueue_goto_actions(
    mut changes: MessageReader<GotoChange>,
    index: Res<InstanceIndex>,
    gothic_script: Res<ScriptResource>,
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
            queued |= queue.push(NpcAction::GoToWaypoint {
                waypoint: waypoint.clone(),
            });
        }
        if !queued {
            warn!(
                "AI_GoToWP waypoint({}) was not queued, npc({}) has no action queue or it is full",
                waypoint, change.instance_id
            );
        }
    }
}

/// Turns `AI_AlignToWP` requests into queued actions targeting the npc's
/// assigned routine waypoint. The action completes without walking when the npc
/// already stands on the waypoint.
pub(crate) fn enqueue_align_actions(
    mut changes: MessageReader<AlignToWpChange>,
    index: Res<InstanceIndex>,
    mut npcs: Query<(&NpcRoutineState, &mut NpcActionQueue)>,
) {
    for change in changes.read() {
        for &entity in index.npcs(change.instance_id) {
            let Ok((routine, mut queue)) = npcs.get_mut(entity) else {
                continue;
            };
            if !queue.push(NpcAction::GoToWaypoint {
                waypoint: routine.assigned_waypoint().clone(),
            }) {
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

/// Turns `AI_Attack` requests into queued actions on their `Npc_SetTarget`
/// target.
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
            queue.push(NpcAction::Attack { target });
            info!(
                "AI_Attack npc({}) queued against target({}) entity({entity})",
                change.instance_id, change.target
            );
        }
    }
}

/// Executes the action at the front of every queue.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(crate) fn run_npc_actions(
    mut commands: Commands,
    way_points: Res<WayPoints>,
    graph: Res<WaypointGraph>,
    spatial_query: SpatialQuery,
    waypoint_nodes: Query<&WaypointNodeIndex>,
    gothic_script: Res<ScriptResource>,
    files: Res<AnimationFiles>,
    mut handles: ResMut<CachedAssetHandles>,
    asset_server: Res<AssetServer>,
    mut desired: MessageWriter<SetDesiredAnimation>,
    targets: Query<(&GlobalTransform, Option<&NpcHealth>)>,
    mut npcs: Query<(
        Entity,
        &GameNpc,
        &mut NpcActionQueue,
        &mut Transform,
        &NpcModel,
        Option<&NpcOverlay>,
        Option<&NpcPath>,
        Option<&AutoAnimSequencer>,
        Option<&mut AnimationController>,
    )>,
) {
    for (entity, npc, mut queue, mut transform, model, overlay, path, sequencer, controller) in
        &mut npcs
    {
        let Some(action) = queue.front().cloned() else {
            // The queue was cleared (`Npc_ClearAIQueue`) or finished; stop any
            // movement still in flight from the dropped action.
            if path.is_some() || sequencer.is_some() {
                debug!("npc({entity}) action queue empty, stopping movement");
                commands.entity(entity).remove::<NpcPath>();
                commands.entity(entity).remove::<AutoAnimSequencer>();
                if let Some(mut controller) = controller {
                    controller.set_root_motion(false);
                    queue_stand_animation(
                        &mut controller,
                        model,
                        overlay,
                        &files,
                        &mut handles,
                        &asset_server,
                    );
                }
            }
            continue;
        };

        match action {
            NpcAction::GoToWaypoint { waypoint } => {
                if !queue.started() {
                    // A replaced front action may still carry the previous path.
                    commands.entity(entity).remove::<NpcPath>();
                    start_goto(
                        entity,
                        &waypoint,
                        &mut queue,
                        &mut transform,
                        &mut commands,
                        &way_points,
                        &graph,
                        &spatial_query,
                        &waypoint_nodes,
                    );
                    continue;
                }

                if path.is_some() {
                    continue;
                }

                // `move_along_path` removed the path, so the npc arrived
                if let Some(waypoint_tr) = way_points.points.get(waypoint.as_str()) {
                    transform.rotation = waypoint_tr.rotation;
                }
                debug!("npc({entity}) finished action goto({waypoint})");
                queue.pop_front();
            }
            NpcAction::Attack { target } => {
                let mut controller = controller;
                let Ok((target_tr, target_health)) = targets.get(target) else {
                    warn!(
                        "npc({}) attack target({target}) is gone, dropping action",
                        npc.instance_id()
                    );
                    drop_attack(
                        &mut commands,
                        entity,
                        &mut queue,
                        controller.as_deref_mut(),
                        sequencer.is_some(),
                        model,
                        overlay,
                        &files,
                        &mut handles,
                        &asset_server,
                    );
                    continue;
                };
                if target_health.is_some_and(|health| health.hitpoints <= 0) {
                    warn!(
                        "npc({}) attack target({target}) is dead, dropping action",
                        npc.instance_id()
                    );
                    drop_attack(
                        &mut commands,
                        entity,
                        &mut queue,
                        controller.as_deref_mut(),
                        sequencer.is_some(),
                        model,
                        overlay,
                        &files,
                        &mut handles,
                        &asset_server,
                    );
                    continue;
                }

                let to_target = target_tr.translation() - transform.translation;
                let horizontal = Vec3::new(to_target.x, 0.0, to_target.z);
                let distance = horizontal.length();
                if distance > ATTACK_MAX_DISTANCE {
                    warn!(
                        "npc({}) attack target is {distance:.1}m away (> {ATTACK_MAX_DISTANCE}m), dropping action",
                        npc.instance_id()
                    );
                    drop_attack(
                        &mut commands,
                        entity,
                        &mut queue,
                        controller.as_deref_mut(),
                        sequencer.is_some(),
                        model,
                        overlay,
                        &files,
                        &mut handles,
                        &asset_server,
                    );
                    continue;
                }

                let Some(mut controller) = controller else {
                    // The visual is not spawned yet, the action waits for it.
                    crate::warn_once!(
                        "npc({}) attack waits for its animation controller (visual not spawned)",
                        npc.instance_id()
                    );
                    continue;
                };

                if !queue.started() {
                    // A preempted walk may still carry its path; the attack
                    // starts with the melee weapon drawn when the npc has one.
                    commands.entity(entity).remove::<NpcPath>();
                    draw_melee_weapon(&gothic_script, npc.instance_id());
                    queue.mark_started();
                    info!(
                        "npc({}) started attack on target({target}) at {distance:.2}m",
                        npc.instance_id()
                    );
                }

                if sequencer.is_none() {
                    // Prepare the sequencer first, desired messages are only
                    // consumed once the component exists.
                    debug!(
                        "npc({}) attack inserts the animation sequencer",
                        npc.instance_id()
                    );
                    controller.set_root_motion(true);
                    commands.entity(entity).insert(AutoAnimSequencer::default());
                    continue;
                }

                let direction = horizontal.normalize_or_zero();
                let direction = (direction.length_squared() > 0.0).then_some(direction);
                let animation = if distance <= ATTACK_RANGE {
                    DesiredAnimation::MeleeAttack
                } else {
                    DesiredAnimation::Run
                };
                trace!(
                    "npc({}) attack desires {animation:?} at {distance:.2}m",
                    npc.instance_id()
                );
                desired.write(SetDesiredAnimation {
                    who: entity,
                    animation,
                    force_change: true,
                    direction,
                });
            }
        }
    }
}

/// Draws the melee weapon of an npc when it has one sheathed, unarmed npcs keep
/// their fists.
fn draw_melee_weapon(gothic_script: &ScriptResource, npc: InstanceId) {
    let mut script = gothic_script.lock();
    let (has_melee, mode) = script.npc_state(npc).map_or((false, None), |npc| {
        (npc.equipment.melee.is_some(), npc.equipment.weapon_mode)
    });
    if !has_melee {
        debug!("npc({npc}) has no melee weapon, attacking unarmed");
        return;
    }
    if mode == Some(WeaponMode::Melee) {
        trace!("npc({npc}) melee weapon already drawn");
        return;
    }
    info!("npc({npc}) drawing its melee weapon for the attack");
    script.draw_weapon(npc, false);
}

/// Drops the attack action at the front of the queue and restores the standing
/// idle when the npc was driven by the animation sequencer.
#[allow(clippy::too_many_arguments)]
fn drop_attack(
    commands: &mut Commands,
    entity: Entity,
    queue: &mut NpcActionQueue,
    controller: Option<&mut AnimationController>,
    has_sequencer: bool,
    model: &NpcModel,
    overlay: Option<&NpcOverlay>,
    files: &AnimationFiles,
    handles: &mut CachedAssetHandles,
    asset_server: &AssetServer,
) {
    commands.entity(entity).remove::<NpcPath>();
    queue.pop_front();
    if !has_sequencer {
        return;
    }
    commands.entity(entity).remove::<AutoAnimSequencer>();
    let Some(controller) = controller else {
        return;
    };
    controller.set_root_motion(false);
    queue_stand_animation(controller, model, overlay, files, handles, asset_server);
}

#[allow(clippy::too_many_arguments, clippy::cognitive_complexity)]
fn start_goto(
    entity: Entity,
    waypoint: &StringCanon,
    queue: &mut NpcActionQueue,
    transform: &mut Transform,
    commands: &mut Commands,
    way_points: &WayPoints,
    graph: &WaypointGraph,
    spatial_query: &SpatialQuery,
    waypoint_nodes: &Query<&WaypointNodeIndex>,
) {
    let Some(waypoint_tr) = way_points.points.get(waypoint.as_str()) else {
        warn!("npc({entity}) action goto({waypoint}), waypoint not found");
        queue.pop_front();
        return;
    };

    let target_position = waypoint_tr.translation + Vec3::Y * NPC_BODY_Y_OFFSET;
    if transform.translation.distance(target_position) <= ARRIVE_DISTANCE {
        transform.rotation = waypoint_tr.rotation;
        debug!("npc({entity}) finished action goto({waypoint}) without walking");
        queue.pop_front();
        return;
    }

    let path = nearest_waypoint_node(spatial_query, waypoint_nodes, graph, transform.translation)
        .and_then(|start| {
            let target = graph.node_by_name(waypoint.as_str())?;
            graph.shortest_path(start, target)
        });
    let Some(path) = path else {
        warn!("npc({entity}) action goto({waypoint}), no path, teleporting");
        teleport(transform, waypoint_tr);
        queue.pop_front();
        return;
    };

    debug!("npc({entity}) starting action goto({waypoint})");
    commands.entity(entity).insert(NpcPath::new(path));
    queue.mark_started();
}

fn teleport(transform: &mut Transform, waypoint: &Transform) {
    let scale = transform.scale;
    transform.translation = waypoint.translation + Vec3::Y * NPC_BODY_Y_OFFSET;
    transform.rotation = waypoint.rotation;
    transform.scale = scale;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn waypoint(name: &str) -> NpcAction {
        NpcAction::GoToWaypoint {
            waypoint: StringCanon::new(name),
        }
    }

    #[test]
    fn queue_replaces_the_last_goto() {
        let mut queue = NpcActionQueue::default();
        assert!(queue.push(waypoint("wp0")));
        assert!(queue.push(waypoint("wp1")));

        assert_eq!(queue.actions.len(), 1);
        let Some(NpcAction::GoToWaypoint { waypoint }) = queue.front() else {
            panic!("front action missing");
        };
        assert_eq!(waypoint.as_str(), "wp1");
    }

    #[test]
    fn replacing_the_active_goto_restarts_it() {
        let mut queue = NpcActionQueue::default();
        queue.push(waypoint("wp0"));
        queue.mark_started();

        queue.push(waypoint("wp1"));

        assert!(!queue.started());
        let Some(NpcAction::GoToWaypoint { waypoint }) = queue.front() else {
            panic!("front action missing");
        };
        assert_eq!(waypoint.as_str(), "wp1");
    }

    #[test]
    fn attack_preempts_queued_actions() {
        let mut queue = NpcActionQueue::default();
        queue.push(waypoint("wp0"));

        assert!(queue.push(NpcAction::Attack {
            target: Entity::PLACEHOLDER,
        }));

        assert_eq!(queue.actions.len(), 1);
        assert!(matches!(queue.front(), Some(NpcAction::Attack { .. })));
        assert!(!queue.started());
    }

    #[test]
    fn repeated_attack_does_not_restart() {
        let mut queue = NpcActionQueue::default();
        let target = Entity::PLACEHOLDER;
        queue.push(NpcAction::Attack { target });
        queue.mark_started();

        assert!(queue.push(NpcAction::Attack { target }));

        assert_eq!(queue.actions.len(), 1);
        assert!(queue.started());
    }

    #[test]
    fn clear_drops_every_action_and_resets_started() {
        let mut queue = NpcActionQueue::default();
        queue.push(waypoint("wp0"));
        queue.push(NpcAction::Attack {
            target: Entity::PLACEHOLDER,
        });
        queue.mark_started();

        queue.clear();

        assert!(queue.front().is_none());
        assert!(!queue.started());
    }
}
