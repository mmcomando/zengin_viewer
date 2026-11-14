use std::collections::HashSet;

use avian3d::prelude::SpatialQuery;
use bevy::prelude::*;

use crate::gameplay::objects::{NpcBodyRootOffset, NpcModel};
use crate::graphics::{
    AnimationController, AutoAnimSequencer, DesiredAnimation, MeleeAttackKind, SetDesiredAnimation,
};
use crate::zengin::common::face_direction_quat;
use crate::zengin::common::{MAX_AI_USE_DISTANCE, MOB_SEARCH_RADIUS};
use crate::zengin::npc_actions::attack::begin_attack_move;
use crate::zengin::npc_actions::goto::start_goto;
use crate::zengin::npc_actions::queue::{NpcAction, NpcActionQueue};
use crate::zengin::npc_routines::{
    AnimationFiles, NPC_BODY_Y_OFFSET, NpcHealth, NpcOverlay, NpcPath, WayPoints,
    queue_named_animation, queue_stand_animation,
};
use crate::zengin::resources::CachedAssetHandles;
use crate::zengin::resources::ZenGinModelComponent;
use crate::zengin::script::component::ScriptResource;
use crate::zengin::script::state::WeaponMode;
use crate::zengin::script::symbol_id::InstanceId;
use crate::zengin::waypoint_graph::{WaypointGraph, WaypointNodeIndex};
use crate::zengin::{FightAiResource, GameMob, GameNpc, MobOccupant, MobRegistry, NpcMobUse};

/// Horizontal distance within which an attacking npc stops chasing and swings.
const ATTACK_RANGE: f32 = 1.0;

/// Horizontal distance beyond which an attack action is abandoned.
const ATTACK_MAX_DISTANCE: f32 = 100.0;

/// `OpenGothic` `MAX_AI_USE_DISTANCE`, in world units (meters).
const MOB_USE_DISTANCE: f32 = MAX_AI_USE_DISTANCE;

/// Executes the action at the front of every queue.
#[allow(
    clippy::type_complexity,
    clippy::too_many_arguments,
    clippy::cognitive_complexity
)]
pub(crate) fn run_npc_actions(
    mut commands: Commands,
    way_points: Res<WayPoints>,
    graph: Res<WaypointGraph>,
    spatial_query: SpatialQuery,
    waypoint_nodes: Query<&WaypointNodeIndex>,
    gothic_script: Res<ScriptResource>,
    fight_ai: Res<FightAiResource>,
    files: Res<AnimationFiles>,
    mut handles: ResMut<CachedAssetHandles>,
    asset_server: Res<AssetServer>,
    mut desired: MessageWriter<SetDesiredAnimation>,
    targets: Query<(&GlobalTransform, Option<&NpcHealth>)>,
    mob_registry: Res<MobRegistry>,
    mut mobs: Query<(
        &mut GameMob,
        &GlobalTransform,
        Option<&MobOccupant>,
        Option<&ZenGinModelComponent>,
    )>,
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
        Option<&NpcMobUse>,
        Option<&NpcBodyRootOffset>,
    )>,
) {
    let mut claimed_mobs = HashSet::new();
    for (
        entity,
        npc,
        mut queue,
        mut transform,
        model,
        overlay,
        path,
        sequencer,
        mut controller,
        npc_mob_use,
        body_root_offset,
    ) in &mut npcs
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
                let mut attack_controller = controller;
                let Ok((target_tr, target_health)) = targets.get(target) else {
                    warn!(
                        "npc({}) attack target({target}) is gone, dropping action",
                        npc.instance_id()
                    );
                    drop_attack(
                        &mut commands,
                        entity,
                        &mut queue,
                        attack_controller.as_deref_mut(),
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
                        attack_controller.as_deref_mut(),
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
                        attack_controller.as_deref_mut(),
                        sequencer.is_some(),
                        model,
                        overlay,
                        &files,
                        &mut handles,
                        &asset_server,
                    );
                    continue;
                }

                let Some(mut controller) = attack_controller else {
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
                    let tactic = gothic_script.lock().npc_fight_tactic(npc.instance_id());
                    let fai_move = begin_attack_move(
                        &mut queue,
                        npc.instance_id(),
                        entity,
                        &fight_ai,
                        &gothic_script,
                    );
                    debug!(
                        "npc({}) attack tactic({tactic:?}) starts FAI move({fai_move:?})",
                        npc.instance_id(),
                    );
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
                let swing = queue
                    .attack()
                    .swings
                    .front()
                    .copied()
                    .unwrap_or(MeleeAttackKind::Default);
                let animation = if distance <= ATTACK_RANGE {
                    DesiredAnimation::MeleeAttack(swing)
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
            NpcAction::UseMob { scheme, state } => {
                if state <= 0 {
                    if let Some(mob_use) = npc_mob_use
                        && let Ok((mut mob, ..)) = mobs.get_mut(mob_use.mob)
                    {
                        mob.state = 0;
                    }
                    release_mob_use(
                        &mut commands,
                        entity,
                        npc_mob_use,
                        controller.as_deref_mut(),
                        model,
                        overlay,
                        &files,
                        &mut handles,
                        &asset_server,
                    );
                    queue.pop_front();
                    continue;
                }

                if let Some(mob_use) = npc_mob_use {
                    if mob_use.scheme == scheme {
                        // The routine loop re-issues AI_UseMob until the body
                        // state reports the use; drop the duplicate.
                        queue.pop_front();
                        continue;
                    }
                    // A different mob use supersedes the current one.
                    commands.entity(mob_use.mob).remove::<MobOccupant>();
                    commands.entity(entity).remove::<NpcMobUse>();
                    if let Ok((mut mob, ..)) = mobs.get_mut(mob_use.mob) {
                        mob.state = 0;
                    }
                }

                let Some(mut controller) = controller else {
                    crate::warn_once!(
                        "npc({}) AI_UseMob({}) waits for its animation controller",
                        npc.instance_id(),
                        scheme
                    );
                    continue;
                };

                let Some(choice) = select_mob(
                    &mut mobs,
                    &mob_registry,
                    &claimed_mobs,
                    scheme.as_str(),
                    transform.translation,
                ) else {
                    debug!(
                        "npc({}) AI_UseMob({}) found no available mob within {MOB_SEARCH_RADIUS}m",
                        npc.instance_id(),
                        scheme
                    );
                    queue.pop_front();
                    continue;
                };
                claimed_mobs.insert(choice.mob);

                let horizontal = choice.use_pos - transform.translation;
                let horizontal = Vec3::new(horizontal.x, 0.0, horizontal.z);
                if horizontal.length() > MOB_USE_DISTANCE {
                    transform.rotation = face_direction_quat(horizontal);
                    if sequencer.is_none() {
                        controller.set_root_motion(true);
                        commands.entity(entity).insert(AutoAnimSequencer::default());
                        continue;
                    }
                    desired.write(SetDesiredAnimation {
                        who: entity,
                        animation: DesiredAnimation::Run,
                        force_change: true,
                        direction: Some(horizontal.normalize_or_zero()),
                    });
                    continue;
                }

                if !choice.ready {
                    // Attachment nodes are extracted once the model asset loads.
                    continue;
                }
                let body_y_offset = body_root_offset
                    .map_or(NPC_BODY_Y_OFFSET, |offset| -offset.0 * transform.scale.y);
                transform.translation = choice.use_pos + Vec3::Y * body_y_offset;
                transform.rotation = choice.use_rot;
                commands.entity(entity).remove::<NpcPath>();
                commands.entity(entity).remove::<AutoAnimSequencer>();
                controller.set_root_motion(false);
                let animation =
                    mob_animation_name(scheme.as_str(), choice.position_tag.as_deref(), state);
                queue_named_animation(
                    &mut controller,
                    model,
                    overlay,
                    &files,
                    &mut handles,
                    &asset_server,
                    &animation,
                );
                commands.entity(choice.mob).insert(MobOccupant(entity));
                commands.entity(entity).insert(NpcMobUse {
                    mob: choice.mob,
                    scheme: scheme.clone(),
                });
                // The mob switches to the state the npc uses it in, which plays
                // its own `<STEM>-S_S<n>` animation.
                if let Ok((mut mob, ..)) = mobs.get_mut(choice.mob)
                    && mob.state != state.max(0)
                {
                    mob.state = state.max(0);
                }
                gothic_script.lock().call_mob_state_hook(
                    npc.instance_id(),
                    &choice.on_state_func,
                    state,
                );
                info!(
                    "npc({}) uses mob({}) focus({}) animation({})",
                    npc.instance_id(),
                    choice.mob,
                    choice.focus_name,
                    animation
                );
                queue.pop_front();
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

/// Available mob selected for an `AI_UseMob` request.
struct MobChoice {
    mob: Entity,
    focus_name: String,
    /// Script hook run when the npc reaches the mob's final state.
    on_state_func: String,
    use_pos: Vec3,
    use_rot: Quat,
    /// `FRONT`/`BACK` of the chosen `ZS_POS*` node, if it has one.
    position_tag: Option<String>,
    distance_squared: f32,
    /// Attachment nodes were extracted, the npc can attach.
    ready: bool,
}

/// Nearest free mob with the requested scheme within [`MOB_SEARCH_RADIUS`].
///
/// Mob requirements (`useWithItem`, `conditionFunc`) are not checked here: the
/// engine only enforces them for the player, npcs use a mob by scheme and a
/// free attachment point alone.
fn select_mob(
    mobs: &mut Query<(
        &mut GameMob,
        &GlobalTransform,
        Option<&MobOccupant>,
        Option<&ZenGinModelComponent>,
    )>,
    registry: &MobRegistry,
    claimed: &HashSet<Entity>,
    scheme: &str,
    npc_pos: Vec3,
) -> Option<MobChoice> {
    // Script strings are canonical lowercased, the registry uses the uppercased
    // scheme derived from the visual name.
    let scheme = scheme.to_uppercase();
    let mut best: Option<MobChoice> = None;
    for &mob_entity in registry.mobs(&scheme) {
        if claimed.contains(&mob_entity) {
            continue;
        }
        let Ok((mob, mob_tr, occupant, model)) = mobs.get_mut(mob_entity) else {
            continue;
        };
        if occupant.is_some() {
            continue;
        }
        let Some((use_pos, use_rot, position_tag, ready)) =
            mob_use_transform(&mob, mob_tr, npc_pos, model.is_some())
        else {
            continue;
        };
        let distance_squared = use_pos.distance_squared(npc_pos);
        if distance_squared > MOB_SEARCH_RADIUS * MOB_SEARCH_RADIUS {
            continue;
        }
        if best
            .as_ref()
            .is_none_or(|best| distance_squared < best.distance_squared)
        {
            best = Some(MobChoice {
                mob: mob_entity,
                focus_name: mob.focus_name.clone(),
                on_state_func: mob.on_state_func.clone(),
                use_pos,
                use_rot,
                position_tag,
                distance_squared,
                ready,
            });
        }
    }
    best
}

/// World transform the npc should use for the mob, using the nearest `ZS_POS*`
/// node. Mobs without attachment nodes fall back to the vob transform.
fn mob_use_transform(
    mob: &GameMob,
    mob_tr: &GlobalTransform,
    npc_pos: Vec3,
    has_model: bool,
) -> Option<(Vec3, Quat, Option<String>, bool)> {
    let Some(positions) = mob.positions.as_ref() else {
        // A model without extracted nodes is still loading; the npc waits at
        // the vob transform and attaches once the asset is scanned.
        return Some((mob_tr.translation(), mob_tr.rotation(), None, !has_model));
    };
    if positions.is_empty() {
        return Some((mob_tr.translation(), mob_tr.rotation(), None, true));
    }
    let mut best: Option<(GlobalTransform, Option<String>, f32)> = None;
    for position in positions {
        let world = mob_tr.mul_transform(position.transform);
        let distance_squared = world.translation().distance_squared(npc_pos);
        if best
            .as_ref()
            .is_none_or(|(_, _, best_dist)| distance_squared < *best_dist)
        {
            best = Some((
                world,
                position_tag(&position.name).map(str::to_string),
                distance_squared,
            ));
        }
    }
    let (world, tag, _) = best?;
    Some((world.translation(), world.rotation(), tag, true))
}

/// `FRONT`/`BACK` animation suffix of a `ZS_POS*` node name.
fn position_tag(name: &str) -> Option<&'static str> {
    if name.ends_with("_FRONT") {
        Some("FRONT")
    } else if name.ends_with("_BACK") {
        Some("BACK")
    } else {
        None
    }
}

/// Npc-side mob animation name, e.g. `S_BENCH_S1` or `S_BEDHIGH_FRONT_S1`.
fn mob_animation_name(scheme: &str, position_tag: Option<&str>, state: i32) -> String {
    let state = state.max(1);
    match position_tag {
        Some(tag) => format!("S_{}_{}_S{state}", scheme.to_uppercase(), tag),
        None => format!("S_{}_S{state}", scheme.to_uppercase()),
    }
}

/// Frees the mob point and restores the standing idle.
#[allow(clippy::too_many_arguments)]
fn release_mob_use(
    commands: &mut Commands,
    npc_entity: Entity,
    mob_use: Option<&NpcMobUse>,
    controller: Option<&mut AnimationController>,
    model: &NpcModel,
    overlay: Option<&NpcOverlay>,
    files: &AnimationFiles,
    handles: &mut CachedAssetHandles,
    asset_server: &AssetServer,
) {
    let Some(mob_use) = mob_use else {
        return;
    };
    debug!("npc({npc_entity}) released mob({})", mob_use.mob);
    commands.entity(mob_use.mob).remove::<MobOccupant>();
    commands.entity(npc_entity).remove::<NpcMobUse>();
    if let Some(controller) = controller {
        controller.set_root_motion(false);
        queue_stand_animation(controller, model, overlay, files, handles, asset_server);
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Resource, Default)]
    struct Selected(Option<Entity>);

    fn select(mut selected: ResMut<Selected>, registry: Res<MobRegistry>, mut mobs: MobQuery) {
        let claimed = HashSet::new();
        selected.0 = select_mob(&mut mobs, &registry, &claimed, "bsanvil", Vec3::ZERO)
            .map(|choice| choice.mob);
    }

    type MobQuery<'w, 's> = Query<
        'w,
        's,
        (
            &'static mut GameMob,
            &'static GlobalTransform,
            Option<&'static MobOccupant>,
            Option<&'static ZenGinModelComponent>,
        ),
    >;

    fn app_with_mob(use_with_item: &str, occupant: Option<Entity>) -> (App, Entity) {
        let mut app = App::new();
        app.init_resource::<MobRegistry>();
        app.init_resource::<Selected>();
        app.add_systems(Update, select);

        let mut mob = app.world_mut().spawn((
            game_mob("BSANVIL", use_with_item),
            Transform::IDENTITY,
            GlobalTransform::IDENTITY,
        ));
        if let Some(occupant) = occupant {
            mob.insert(MobOccupant(occupant));
        }
        let mob = mob.id();
        app.world_mut()
            .resource_mut::<MobRegistry>()
            .insert("BSANVIL".to_string(), mob);
        app.update();
        (app, mob)
    }

    fn game_mob(scheme: &str, use_with_item: &str) -> GameMob {
        GameMob {
            index: 0,
            vob_name: String::new(),
            focus_name: String::new(),
            scheme: scheme.to_string(),
            visual_stem: scheme.to_uppercase(),
            state_num: 1,
            use_with_item: use_with_item.to_string(),
            on_state_func: String::new(),
            condition_func: String::new(),
            rewind: false,
            positions: None,
            state: 0,
        }
    }

    #[test]
    fn select_mob_ignores_the_use_item_requirement() {
        // The engine only enforces `useWithItem` for the player, an npc uses an
        // anvil without carrying the hot blank.
        let (app, mob) = app_with_mob("ItMiSwordRawHot", None);

        assert_eq!(app.world().resource::<Selected>().0, Some(mob));
    }

    #[test]
    fn select_mob_skips_an_occupied_mob() {
        let (app, _) = app_with_mob("", Some(Entity::PLACEHOLDER));

        assert_eq!(app.world().resource::<Selected>().0, None);
    }
}
