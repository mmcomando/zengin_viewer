use bevy::prelude::*;

use crate::zengin::GameNpc;
use crate::zengin::common::horizontal_offset;
use crate::zengin::npc_actions::action::ExecutingActionMarker;
use crate::zengin::npc_routines::{
    NpcWalkMode, queue_root_motion_animation, queue_stand_animation,
};
use crate::zengin::{
    AnimationController, AutoAnimSequencer, DesiredAnimation, SetDesiredAnimation,
};
use dev_util::perf_debug;

/// Walks the npc to another entity, following it while it moves.
#[derive(Component)]
#[require(ExecutingActionMarker)]
pub(crate) struct GoToNpcAction {
    pub(crate) target: Entity,
}

/// Horizontal distance in Bevy meters within which the npc counts as having
/// reached the target.
const ARRIVE_DISTANCE: f32 = 1.0;

pub(crate) fn start_goto_npc_action(
    mut commands: Commands,
    targets: Query<&GlobalTransform>,
    mut npcs: Query<(Entity, &GoToNpcAction, &mut AnimationController), Added<GoToNpcAction>>,
) {
    let perf = perf_debug!();
    for (entity, action, mut controller) in &mut npcs {
        perf.increment_tag("actions", 1);
        if targets.get(action.target).is_err() {
            warn!(
                "npc entity({entity}) goto npc target({}) is gone, dropping action",
                action.target
            );
            commands.entity(entity).remove::<ExecutingActionMarker>();
            continue;
        }
        controller.set_root_motion(true);
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn run_goto_npc_action(
    mut commands: Commands,
    targets: Query<&GlobalTransform>,
    mut desired: MessageWriter<SetDesiredAnimation>,
    mut npcs: Query<(
        Entity,
        &GameNpc,
        &GoToNpcAction,
        &Transform,
        &mut AnimationController,
        Option<&NpcWalkMode>,
    )>,
) {
    let perf = perf_debug!();
    for (entity, npc, action, transform, mut controller, walk_mode) in &mut npcs {
        perf.increment_tag("actions", 1);
        let Ok(target_tr) = targets.get(action.target) else {
            warn!(
                "npc({}) goto npc target({}) is gone, dropping action",
                npc.instance_id(),
                action.target
            );
            commands.entity(entity).remove::<ExecutingActionMarker>();
            continue;
        };
        let horizontal = horizontal_offset(transform.translation, target_tr.translation());
        let distance = horizontal.length();
        if distance <= ARRIVE_DISTANCE {
            debug!(
                "npc({}) reached goto npc target({})",
                npc.instance_id(),
                action.target
            );
            commands.entity(entity).remove::<ExecutingActionMarker>();
            continue;
        }
        let direction = horizontal.normalize_or_zero();
        let direction = (direction.length_squared() > 0.0).then_some(direction);
        let animation = match walk_mode.map_or(0, |mode| mode.0) {
            1 => DesiredAnimation::Walk,
            2 => DesiredAnimation::Sneak,
            _ => DesiredAnimation::Run,
        };
        queue_root_motion_animation(&mut desired, entity, &mut controller, animation, direction);
    }
}

pub(crate) fn end_goto_npc_action(
    mut removed: RemovedComponents<GoToNpcAction>,
    mut npcs: Query<(&mut AutoAnimSequencer, &mut AnimationController)>,
) {
    let perf = perf_debug!();
    for entity in removed.read() {
        perf.increment_tag("actions", 1);
        let Ok((mut sequencer, mut controller)) = npcs.get_mut(entity) else {
            continue;
        };
        controller.set_root_motion(false);
        queue_stand_animation(&mut sequencer);
    }
}
