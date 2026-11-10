use avian3d::prelude::SpatialQuery;
use bevy::prelude::*;

use crate::gameplay::objects::NpcModel;
use crate::graphics::{
    AnimationController, AutoAnimSequencer, DesiredAnimation, SetDesiredAnimation,
};
use crate::zengin::GameNpc;
use crate::zengin::npc_actions::goto::start_goto;
use crate::zengin::npc_actions::queue::{NpcAction, NpcActionQueue};
use crate::zengin::npc_routines::{
    AnimationFiles, NpcHealth, NpcOverlay, NpcPath, WayPoints, queue_stand_animation,
};
use crate::zengin::resources::CachedAssetHandles;
use crate::zengin::script::component::ScriptResource;
use crate::zengin::script::state::WeaponMode;
use crate::zengin::script::symbol_id::InstanceId;
use crate::zengin::waypoint_graph::{WaypointGraph, WaypointNodeIndex};

/// Horizontal distance within which an attacking npc stops chasing and swings.
const ATTACK_RANGE: f32 = 1.0;

/// Horizontal distance beyond which an attack action is abandoned.
const ATTACK_MAX_DISTANCE: f32 = 100.0;

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
