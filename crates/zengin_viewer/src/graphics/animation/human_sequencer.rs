//! Human animation sequencer: the message driven state machine replacing the
//! polling based locomotion sequencer and the timer based bow attack for the
//! player.
//!
// The plugin is currently disabled in favour of the auto anim sequencer; the
// module is kept (tests included) until the sequencer migration is done.
#![allow(dead_code)]
//! The sequencer owns the [`AnimationController`] of the entities it drives.
//! Desired actions arrive as [`ChangeDesiredAction`] messages (written every
//! frame by the input bridge, deduplicated here), animation progression
//! arrives as [`AnimationEnds`]/[`AnimationTrigger`] messages from the
//! playback. Every frame the system computes `desired_state` from the desired
//! action and walks the transition graph between the current and the desired
//! sequence family (see `HumanAnimationSequence`), queueing animations via
//! path-derived handles ([`human_animation_handle`]).
//!
//! With the sequencer active the player moves by root motion: the horizontal
//! velocity of the physics body is zeroed and the playback moves the entity
//! along the animation's root movement, so the model rotation (driven here
//! from the movement intent) steers the movement direction.

use avian3d::prelude::LinearVelocity;
use bevy::prelude::*;
use leafwing_input_manager::prelude::ActionState;

use crate::gameplay::character::PlayerPhysicsBodyMarker;
use crate::gameplay::objects::PlayerCharacter;
use crate::input::actions::{GameplayAction, GameplayInputMarker};
use crate::zengin::{
    ArrowShot, MaterialHandles, NpcWeaponMode, Selection, WayPoints, WeaponMode,
    face_direction_quat, get_animation_path,
};

use super::components::{AnimationController, AnimationEnds};
use super::data::AnimationData;
use super::human::{
    ArmedState, DesiredAction, FireBowSequence, HumanAnimationSequence, HumanAnimationSequencer,
    PreferedLocomotion, RunSequence, SequenceFamily, WalkSequence,
};

/// Yaw turn speed of the model while following the movement intent, radians
/// per second.
const TURN_SPEED: f32 = 3.0;

/// Marks a physics body whose horizontal movement is driven by animation root
/// motion instead of the character controller acceleration.
#[derive(Component, Debug)]
pub struct RootMotionMovement;

/// Sets the desired action of a sequencer entity.
#[derive(Message, Debug, Clone)]
pub struct ChangeDesiredAction {
    /// Animation model entity carrying the [`HumanAnimationSequencer`].
    pub who: Entity,
    pub action: DesiredAction,
}

pub struct HumanSequencerPlugin;

impl Plugin for HumanSequencerPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<ChangeDesiredAction>();
        app.add_systems(
            Update,
            (
                add_human_sequencer,
                send_desired_actions,
                run_sequencer.before(super::locomotion::update_animations),
            )
                .chain(),
        );
    }
}

/// Adds the sequencer to the player animation model and marks its physics
/// body as root motion driven. Also survives visual respawns, like the bow
/// attack component did.
fn add_human_sequencer(
    mut commands: Commands,
    mut players: Query<
        (Entity, &mut AnimationController, Option<&ChildOf>),
        (Added<AnimationController>, With<PlayerCharacter>),
    >,
) {
    for (entity, mut controller, parent) in &mut players {
        // The sequencer drives the player by root motion
        controller.should_move_entity = true;
        commands
            .entity(entity)
            .insert(HumanAnimationSequencer::default());
        if let Some(parent) = parent {
            commands.entity(parent.parent()).insert(RootMotionMovement);
        }
    }
}

/// Writes the desired action of the player every frame. Firing (ranged
/// weapon drawn and attack pressed) wins over movement, movement wins over
/// standing.
fn send_desired_actions(
    action_state: Single<&ActionState<GameplayAction>, With<GameplayInputMarker>>,
    players: Query<
        (Entity, Option<&NpcWeaponMode>, &GlobalTransform),
        (With<PlayerCharacter>, With<HumanAnimationSequencer>),
    >,
    camera: Single<&Transform, With<Camera3d>>,
    selection: Res<Selection>,
    transforms: Query<&GlobalTransform>,
    mut writer: MessageWriter<ChangeDesiredAction>,
) {
    let Ok((entity, weapon_mode, player_tr)) = players.single() else {
        return;
    };

    let mode = weapon_mode.map_or(WeaponMode::Fists, |mode| mode.0);
    let action = if mode == WeaponMode::Ranged && action_state.pressed(&GameplayAction::Interact) {
        // Aim at the selected npc, fall back to the current facing direction.
        let target = selection
            .target
            .and_then(|target| transforms.get(target).ok())
            .map(|target| target.translation())
            .unwrap_or_else(|| player_tr.translation() + (player_tr.rotation() * Vec3::Z) * 20.0);
        DesiredAction::RangedFireLocation(target)
    } else {
        let direction = action_state.axis_pair(&GameplayAction::Move);
        if direction.length_squared() >= 0.1 {
            // Movement is relative to the camera view: forward input walks away
            // from the camera. This holds for every camera mode (third person
            // orbit and free camera alike), the camera rotation is what the
            // player sees. The physics body rotation stays identity and is
            // irrelevant to movement.
            let forward = cam_tr_horizontal_forward(&camera);
            let right = cam_tr_horizontal_right(&camera);
            let dir = right * direction.x + forward * direction.y;
            if dir.length_squared() < 0.0001 {
                return;
            }
            DesiredAction::GoDir(face_direction_quat(dir.normalize()))
        } else {
            DesiredAction::Stand
        }
    };

    writer.write(ChangeDesiredAction {
        who: entity,
        action,
    });
}

/// Horizontal view direction of the camera.
fn cam_tr_horizontal_forward(camera: &Transform) -> Vec3 {
    let forward = *camera.forward();
    Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero()
}

/// Horizontal screen-right direction of the camera.
fn cam_tr_horizontal_right(camera: &Transform) -> Vec3 {
    let right = *camera.right();
    Vec3::new(right.x, 0.0, right.z).normalize_or_zero()
}

/// Desired sequence state derived from the desired action.
fn desired_state(sequencer: &HumanAnimationSequencer) -> HumanAnimationSequence {
    match &sequencer.desired_action {
        DesiredAction::Stand => HumanAnimationSequence::Stand,
        DesiredAction::RangedFireLocation(_) => {
            HumanAnimationSequence::FireBowSequence(FireBowSequence::Reload)
        }
        DesiredAction::GoDir(_) | DesiredAction::GoWaypoint(_) => {
            match sequencer.prefered_locomotion {
                PreferedLocomotion::Run => HumanAnimationSequence::RunSequence(RunSequence::Loop),
                PreferedLocomotion::Walk => {
                    HumanAnimationSequence::WalkSequence(WalkSequence::Loop)
                }
            }
        }
    }
}

/// Advances the sequencer: consumes desired actions and animation events,
/// walks the transition graph, rotates the model toward the movement intent
/// and keeps the root motion driven body velocity free of horizontal
/// acceleration.
#[allow(clippy::type_complexity)]
fn run_sequencer(
    time: Res<Time>,
    asset_server: Res<AssetServer>,
    mut handles: ResMut<MaterialHandles>,
    way_points: Res<WayPoints>,
    mut actions: MessageReader<ChangeDesiredAction>,
    mut ends: MessageReader<AnimationEnds>,
    mut arrows: MessageWriter<ArrowShot>,
    mut sequencers: Query<(
        Entity,
        &mut HumanAnimationSequencer,
        &mut AnimationController,
        &mut Transform,
        Option<&ChildOf>,
        Option<&NpcWeaponMode>,
        &GlobalTransform,
    )>,
    mut velocities: Query<
        (&mut LinearVelocity, &Transform),
        (
            With<PlayerPhysicsBodyMarker>,
            Without<HumanAnimationSequencer>,
        ),
    >,
) {
    // Read all messages once; per-entity filtering below needs repeated access
    let actions: Vec<_> = actions.read().cloned().collect();
    let ends: Vec<_> = ends.read().cloned().collect();
    let delta = time.delta_secs();

    for (entity, mut seq, mut controller, mut tr, parent, weapon_mode, global_tr) in &mut sequencers
    {
        let armed = weapon_mode.map_or(ArmedState::Unarmed, |mode| match mode.0 {
            WeaponMode::Fists => ArmedState::Unarmed,
            WeaponMode::Melee => ArmedState::Melee,
            WeaponMode::Ranged => ArmedState::Ranged,
        });

        for action in &actions {
            if action.who == entity && seq.desired_action != action.action {
                seq.desired_action = action.action.clone();
            }
        }
        seq.desired_state = desired_state(&seq);

        // Animation ends of the currently played animation advance the state
        // machine within its family (Start -> Loop -> End chains). At most one
        // end is processed per frame; several wraps in one step (long pauses)
        // resolve over the following frames.
        let current_ends: Vec<Handle<AnimationData>> = ends
            .iter()
            .filter(|end| end.who == entity && end.what_animation == controller.current)
            .map(|end| end.what_animation.clone())
            .collect();
        let ends_now = !current_ends.is_empty();
        if ends_now {
            on_animation_ends(
                entity,
                &mut seq,
                &mut controller,
                &mut handles,
                &asset_server,
                armed,
                &mut arrows,
            );
        }

        // Transitions between sequence families happen independent of
        // animation events; one-shot transitions wait for their animation to
        // end (handled above).
        if seq.current_state.family() != seq.desired_state.family() {
            match seq.current_state {
                HumanAnimationSequence::Stand => {
                    transition_to_desired(
                        &mut seq,
                        &mut controller,
                        &mut handles,
                        &asset_server,
                        armed,
                        true,
                    );
                }
                // Loops can be left right away; when the animation is about to
                // wrap anyway, leave at the clean cycle boundary instead.
                HumanAnimationSequence::RunSequence(RunSequence::Loop)
                | HumanAnimationSequence::WalkSequence(WalkSequence::Loop)
                | HumanAnimationSequence::FireBowSequence(FireBowSequence::Reload) => {
                    leave_loop(
                        &mut seq,
                        &mut controller,
                        &mut handles,
                        &asset_server,
                        armed,
                        ends_now,
                    );
                }

                // One-shot transitions wait for their AnimationEnds.
                _ => {}
            }
        }

        // Rotate the model toward the movement or aim direction; standing
        // keeps the last facing.
        let target_rotation = match &seq.desired_action {
            DesiredAction::GoDir(quat) => Some(*quat),
            DesiredAction::GoWaypoint(name) => way_points.points.get(name).map(|way_point| {
                let dir = way_point.translation - global_tr.translation();
                face_direction_quat(Vec3::new(dir.x, 0.0, dir.z))
            }),
            DesiredAction::RangedFireLocation(pos) => {
                let dir = *pos - global_tr.translation();
                Some(face_direction_quat(Vec3::new(dir.x, 0.0, dir.z)))
            }
            DesiredAction::Stand => None,
        };
        if let Some(target_rotation) = target_rotation {
            tr.rotation = tr
                .rotation
                .slerp(target_rotation, (TURN_SPEED * delta).min(1.0));
            let (yaw, _, _) = tr.rotation.to_euler(EulerRot::YXZ);
            let (target_yaw, _, _) = target_rotation.to_euler(EulerRot::YXZ);
            debug!(
                "Sequencer rotation player({entity}) yaw({yaw}) target_yaw({target_yaw}) pos({})",
                global_tr.translation()
            );
        }

        // Root motion drives the horizontal position of the body, the
        // character controller only supplies gravity and jumps.
        if let Some(parent) = parent
            && let Ok((mut velocity, body_tr)) = velocities.get_mut(parent.parent())
        {
            if velocity.x != 0.0 || velocity.z != 0.0 {
                let (body_yaw, _, _) = body_tr.rotation.to_euler(EulerRot::YXZ);
                debug!(
                    "Sequencer zeroed horizontal velocity player({entity}) body_yaw({body_yaw}) vel({:?})",
                    **velocity
                );
            }
            velocity.x = 0.0;
            velocity.z = 0.0;
        }
    }
}

/// Leaves a looping sequence at the animation cycle boundary when one is
/// imminent, otherwise cuts the loop.
fn leave_loop(
    seq: &mut HumanAnimationSequencer,
    controller: &mut AnimationController,
    handles: &mut MaterialHandles,
    asset_server: &AssetServer,
    armed: ArmedState,
    ends_now: bool,
) {
    let end_state = match seq.current_state {
        HumanAnimationSequence::RunSequence(_) => {
            HumanAnimationSequence::RunSequence(RunSequence::End)
        }
        HumanAnimationSequence::WalkSequence(_) => {
            HumanAnimationSequence::WalkSequence(WalkSequence::End)
        }
        HumanAnimationSequence::FireBowSequence(_) => {
            HumanAnimationSequence::FireBowSequence(FireBowSequence::End)
        }
        _ => return,
    };
    seq.current_state = end_state;
    queue_animation(seq, controller, handles, asset_server, armed, !ends_now);
}

/// Queues the entry animation of the desired sequence family.
fn transition_to_desired(
    seq: &mut HumanAnimationSequencer,
    controller: &mut AnimationController,
    handles: &mut MaterialHandles,
    asset_server: &AssetServer,
    armed: ArmedState,
    interrupt: bool,
) {
    match seq.desired_state.family() {
        SequenceFamily::Stand => {
            seq.current_state = seq.desired_state;
            queue_animation(seq, controller, handles, asset_server, armed, interrupt);
        }
        SequenceFamily::Run => {
            seq.current_state = HumanAnimationSequence::RunSequence(RunSequence::Start);
            queue_animation(seq, controller, handles, asset_server, armed, interrupt);
        }
        SequenceFamily::Walk => {
            seq.current_state = HumanAnimationSequence::WalkSequence(WalkSequence::Start);
            queue_animation(seq, controller, handles, asset_server, armed, interrupt);
        }
        SequenceFamily::FireBow => {
            seq.current_state = HumanAnimationSequence::FireBowSequence(FireBowSequence::Start);
            queue_animation(seq, controller, handles, asset_server, armed, interrupt);
        }
    }
}

/// Animation end of the current animation: advance within the sequence family
/// or hand over to the desired family.
fn on_animation_ends(
    entity: Entity,
    seq: &mut HumanAnimationSequencer,
    controller: &mut AnimationController,
    handles: &mut MaterialHandles,
    asset_server: &AssetServer,
    armed: ArmedState,
    arrows: &mut MessageWriter<ArrowShot>,
) {
    if seq.current_state.family() != seq.desired_state.family() {
        // Looping states leave via their End transition (clean when at the
        // cycle boundary, which an ends message is); one-shots hand over
        // directly.
        if matches!(
            seq.current_state,
            HumanAnimationSequence::RunSequence(RunSequence::Loop)
                | HumanAnimationSequence::WalkSequence(WalkSequence::Loop)
                | HumanAnimationSequence::FireBowSequence(FireBowSequence::Reload)
        ) {
            leave_loop(seq, controller, handles, asset_server, armed, true);
        } else {
            transition_to_desired(seq, controller, handles, asset_server, armed, false);
        }
        return;
    }

    match seq.current_state {
        // The start transition finished: run
        HumanAnimationSequence::RunSequence(RunSequence::Start)
        | HumanAnimationSequence::WalkSequence(WalkSequence::Start) => {
            enter_loop(seq, controller, handles, asset_server, armed);
        }
        // The stop transition finished: stand or start the next sequence
        HumanAnimationSequence::RunSequence(RunSequence::End)
        | HumanAnimationSequence::WalkSequence(WalkSequence::End) => {
            transition_to_desired(seq, controller, handles, asset_server, armed, false);
        }
        // The aim in transition finished: loop loading and firing arrows
        HumanAnimationSequence::FireBowSequence(FireBowSequence::Start) => {
            if seq.desired_state.family() == SequenceFamily::FireBow {
                seq.current_state =
                    HumanAnimationSequence::FireBowSequence(FireBowSequence::Reload);
            } else {
                seq.current_state = HumanAnimationSequence::FireBowSequence(FireBowSequence::End);
            }
            queue_animation(seq, controller, handles, asset_server, armed, false);
        }
        // The reload loop wrapped: the arrow leaves the bow
        HumanAnimationSequence::FireBowSequence(FireBowSequence::Reload) => {
            if seq.desired_state.family() == SequenceFamily::FireBow {
                // The current animation keeps looping; queueing is a no-op
                // while it is already the current one.
                queue_animation(seq, controller, handles, asset_server, armed, false);
                arrows.write(ArrowShot { shooter: entity });
            } else {
                leave_loop(seq, controller, handles, asset_server, armed, true);
            }
        }
        // Loops and standing stay as they are
        _ => {}
    }
}

/// Enters the loop state of the current sequence family; the loop animation
/// starts at the cycle boundary of the finished transition.
fn enter_loop(
    seq: &mut HumanAnimationSequencer,
    controller: &mut AnimationController,
    handles: &mut MaterialHandles,
    asset_server: &AssetServer,
    armed: ArmedState,
) {
    match seq.current_state {
        HumanAnimationSequence::RunSequence(_) => {
            seq.current_state = HumanAnimationSequence::RunSequence(RunSequence::Loop);
        }
        HumanAnimationSequence::WalkSequence(_) => {
            seq.current_state = HumanAnimationSequence::WalkSequence(WalkSequence::Loop);
        }
        _ => return,
    }
    queue_animation(seq, controller, handles, asset_server, armed, false);
}

/// Queues the animation of `seq.current_state`. The handle comes from the
/// shared [`MaterialHandles`] cache (same mechanism the npc animations use);
/// the sequencer animations are preloaded at startup.
fn queue_animation(
    seq: &mut HumanAnimationSequencer,
    controller: &mut AnimationController,
    handles: &mut MaterialHandles,
    asset_server: &AssetServer,
    armed: ArmedState,
    interrupt: bool,
) {
    let name = seq.current_state.animation_name(armed);
    let handle = handles.get_animation_handle(asset_server, &get_animation_path("HUMANS", name));
    if controller.current == handle {
        return;
    }
    controller.next = Some(handle);
    controller.interrupt_current = interrupt;
}

#[cfg(test)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;

    use super::*;
    use crate::gameplay::distance::DistanceNear;

    /// A minimal asset server for the sequencer tests; assets are never
    /// actually loaded, only handles are compared.
    fn test_asset_server() -> AssetServer {
        let mut app = App::new();
        app.add_plugins((TaskPoolPlugin::default(), AssetPlugin::default()));
        app.init_asset::<AnimationData>();
        app.world().resource::<AssetServer>().clone()
    }

    fn animation_handle(world: &mut World, name: &str) -> Handle<AnimationData> {
        let server = world.resource::<AssetServer>().clone();
        let path = get_animation_path("HUMANS", name);
        world
            .resource_mut::<MaterialHandles>()
            .get_animation_handle(&server, &path)
    }

    fn run_sequencer_world(sequencer: HumanAnimationSequencer) -> (World, Entity) {
        let mut world = World::new();
        world.insert_resource(Time::<()>::default());
        world.init_resource::<Messages<AnimationEnds>>();
        world.init_resource::<Messages<ChangeDesiredAction>>();
        world.init_resource::<Messages<ArrowShot>>();
        world.insert_resource(Selection::default());
        world.insert_resource(WayPoints::default());
        world.insert_resource(test_asset_server());
        world.insert_resource(MaterialHandles::default());
        let idle = animation_handle(&mut world, "S_RUN");
        let model = world
            .spawn((
                DistanceNear::default(),
                GlobalTransform::default(),
                Transform::default(),
                AnimationController {
                    current: idle,
                    next: None,
                    interrupt_current: false,
                    should_move_entity: false,
                },
                sequencer,
                PlayerCharacter,
            ))
            .id();
        (world, model)
    }

    fn send_action(world: &mut World, who: Entity, action: DesiredAction) {
        world
            .resource_mut::<Messages<ChangeDesiredAction>>()
            .write(ChangeDesiredAction { who, action });
    }

    fn send_ends(world: &mut World, who: Entity, what: Handle<AnimationData>) {
        world
            .resource_mut::<Messages<AnimationEnds>>()
            .write(AnimationEnds {
                who,
                what_animation: what,
            });
    }

    fn current_state(world: &World, model: Entity) -> HumanAnimationSequence {
        world
            .get::<HumanAnimationSequencer>(model)
            .unwrap()
            .current_state
    }

    fn queued_next(world: &World, model: Entity) -> Option<Handle<AnimationData>> {
        world
            .get::<AnimationController>(model)
            .unwrap()
            .next
            .clone()
    }

    #[test]
    fn standing_starts_running_on_go_dir() {
        let (mut world, model) = run_sequencer_world(HumanAnimationSequencer::default());
        let run_start = animation_handle(&mut world, "T_RUN_2_RUNL");

        send_action(&mut world, model, DesiredAction::GoDir(Quat::IDENTITY));
        world.run_system_once(run_sequencer).unwrap();

        assert_eq!(
            current_state(&world, model),
            HumanAnimationSequence::RunSequence(RunSequence::Start)
        );
        assert_eq!(queued_next(&world, model), Some(run_start));
    }

    #[test]
    fn run_start_advances_to_loop_on_animation_end() {
        let (mut world, model) = run_sequencer_world(HumanAnimationSequencer {
            current_state: HumanAnimationSequence::RunSequence(RunSequence::Start),
            desired_state: HumanAnimationSequence::RunSequence(RunSequence::Loop),
            desired_action: DesiredAction::GoDir(Quat::IDENTITY),
            ..HumanAnimationSequencer::default()
        });
        let run_start = animation_handle(&mut world, "T_RUN_2_RUNL");
        let run_loop = animation_handle(&mut world, "S_RUNL");
        world.get_mut::<AnimationController>(model).unwrap().current = run_start.clone();

        send_ends(&mut world, model, run_start);
        world.run_system_once(run_sequencer).unwrap();

        assert_eq!(
            current_state(&world, model),
            HumanAnimationSequence::RunSequence(RunSequence::Loop)
        );
        assert_eq!(queued_next(&world, model), Some(run_loop));
    }

    #[test]
    fn run_loop_stops_via_end_on_stand() {
        let (mut world, model) = run_sequencer_world(HumanAnimationSequencer {
            current_state: HumanAnimationSequence::RunSequence(RunSequence::Loop),
            desired_state: HumanAnimationSequence::RunSequence(RunSequence::Loop),
            desired_action: DesiredAction::GoDir(Quat::IDENTITY),
            ..HumanAnimationSequencer::default()
        });
        let run_loop = animation_handle(&mut world, "S_RUNL");
        let run_end = animation_handle(&mut world, "T_RUNL_2_RUN");
        world.get_mut::<AnimationController>(model).unwrap().current = run_loop.clone();

        // Loop wrap while still running: stay in the loop
        send_ends(&mut world, model, run_loop.clone());
        world.run_system_once(run_sequencer).unwrap();
        assert_eq!(queued_next(&world, model), None);

        // Loop wrap with a stand desire: leave at the cycle boundary
        send_action(&mut world, model, DesiredAction::Stand);
        send_ends(&mut world, model, run_loop.clone());
        world.run_system_once(run_sequencer).unwrap();
        assert_eq!(
            current_state(&world, model),
            HumanAnimationSequence::RunSequence(RunSequence::End)
        );
        assert_eq!(queued_next(&world, model), Some(run_end));
    }

    #[test]
    fn run_end_returns_to_stand_on_animation_end() {
        let (mut world, model) = run_sequencer_world(HumanAnimationSequencer {
            current_state: HumanAnimationSequence::RunSequence(RunSequence::End),
            desired_state: HumanAnimationSequence::RunSequence(RunSequence::Loop),
            desired_action: DesiredAction::GoDir(Quat::IDENTITY),
            ..HumanAnimationSequencer::default()
        });
        let run_end = animation_handle(&mut world, "T_RUNL_2_RUN");
        let idle = animation_handle(&mut world, "S_RUN");
        world.get_mut::<AnimationController>(model).unwrap().current = run_end.clone();

        // Desired stand while ending: the end animation hands over to stand
        send_action(&mut world, model, DesiredAction::Stand);
        send_ends(&mut world, model, run_end);
        world.run_system_once(run_sequencer).unwrap();

        assert_eq!(current_state(&world, model), HumanAnimationSequence::Stand);
        assert_eq!(queued_next(&world, model), Some(idle));
    }

    #[test]
    fn firing_starts_then_shoots_on_reload_end() {
        let (mut world, model) = run_sequencer_world(HumanAnimationSequencer::default());
        let aim_in = animation_handle(&mut world, "T_BOWRUN_2_BOWAIM");
        let reload = animation_handle(&mut world, "T_BOWRELOAD");

        send_action(
            &mut world,
            model,
            DesiredAction::RangedFireLocation(Vec3::ONE),
        );
        world.run_system_once(run_sequencer).unwrap();
        assert_eq!(
            current_state(&world, model),
            HumanAnimationSequence::FireBowSequence(FireBowSequence::Start)
        );
        assert_eq!(queued_next(&world, model), Some(aim_in.clone()));

        // Aim in finished: the reload loop starts, no shot yet
        world.get_mut::<AnimationController>(model).unwrap().current = aim_in.clone();
        send_ends(&mut world, model, aim_in);
        world.run_system_once(run_sequencer).unwrap();
        assert_eq!(
            current_state(&world, model),
            HumanAnimationSequence::FireBowSequence(FireBowSequence::Reload)
        );
        assert_eq!(queued_next(&world, model), Some(reload.clone()));
        assert_eq!(
            world
                .resource::<Messages<ArrowShot>>()
                .iter_current_update_messages()
                .count(),
            0
        );

        // The reload loop wrapped: the arrow fires and the loop continues
        world.get_mut::<AnimationController>(model).unwrap().current = reload.clone();
        send_ends(&mut world, model, reload);
        world.run_system_once(run_sequencer).unwrap();
        assert_eq!(
            current_state(&world, model),
            HumanAnimationSequence::FireBowSequence(FireBowSequence::Reload)
        );
        assert_eq!(
            world
                .resource::<Messages<ArrowShot>>()
                .iter_current_update_messages()
                .count(),
            1
        );
    }

    #[test]
    fn firing_stops_via_aim_out() {
        let (mut world, model) = run_sequencer_world(HumanAnimationSequencer {
            current_state: HumanAnimationSequence::FireBowSequence(FireBowSequence::Reload),
            desired_state: HumanAnimationSequence::FireBowSequence(FireBowSequence::Reload),
            desired_action: DesiredAction::RangedFireLocation(Vec3::ONE),
            ..HumanAnimationSequencer::default()
        });
        let aim_out = animation_handle(&mut world, "BOWRUN_2_BOWRUNL");

        // Released mid cycle: leave the loop right away
        send_action(&mut world, model, DesiredAction::Stand);
        world.run_system_once(run_sequencer).unwrap();

        assert_eq!(
            current_state(&world, model),
            HumanAnimationSequence::FireBowSequence(FireBowSequence::End)
        );
        assert_eq!(queued_next(&world, model), Some(aim_out));
    }

    #[test]
    fn firing_stops_at_reload_boundary_via_aim_out() {
        let (mut world, model) = run_sequencer_world(HumanAnimationSequencer {
            current_state: HumanAnimationSequence::FireBowSequence(FireBowSequence::Reload),
            desired_state: HumanAnimationSequence::FireBowSequence(FireBowSequence::Reload),
            desired_action: DesiredAction::RangedFireLocation(Vec3::ONE),
            ..HumanAnimationSequencer::default()
        });
        let reload = animation_handle(&mut world, "T_BOWRELOAD");
        let aim_out = animation_handle(&mut world, "T_BOWRUN_2_BOWRUNL");

        // Release and the loop wrap in the same step: leave at the boundary,
        // no arrow for the released shot
        send_action(&mut world, model, DesiredAction::Stand);
        world.get_mut::<AnimationController>(model).unwrap().current = reload.clone();
        send_ends(&mut world, model, reload);
        world.run_system_once(run_sequencer).unwrap();
        assert_eq!(
            current_state(&world, model),
            HumanAnimationSequence::FireBowSequence(FireBowSequence::End)
        );
        assert_eq!(queued_next(&world, model), Some(aim_out));
        assert_eq!(
            world
                .resource::<Messages<ArrowShot>>()
                .iter_current_update_messages()
                .count(),
            0
        );
    }
}
