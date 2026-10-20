//! Auto animation sequencer: data driven animation selection through the
//! [`AnimationGraph`].
//!
//! Unlike [`super::human_sequencer`] (currently disabled) the sequencer does
//! not know about sequence families or loops. It only knows the *desired*
//! animation (arriving as a [`SetDesiredAnimation`] message, written every
//! frame from the move input) and walks the animation graph from the
//! currently played animation towards it: on every [`AnimationEnds`] of the
//! current animation it queries the graph for the next animation on the path
//! and queues it; when the graph offers no path it cuts straight to the
//! desired animation.
//!
//! Movement still works through root motion (`should_move_entity`), the
//! model is rotated smoothly towards the move direction.

use std::path::Path;

use avian3d::prelude::LinearVelocity;
use bevy::prelude::*;
use leafwing_input_manager::prelude::ActionState;

use crate::gameplay::character::PlayerPhysicsBodyMarker;
use crate::gameplay::objects::PlayerCharacter;
use crate::input::actions::{GameplayAction, GameplayInputMarker};
use crate::zengin::{
    AnimationGraph, AnimationIndex, ArrowShot, MaterialHandles, NpcWeaponMode, Selection,
    WeaponMode, animation_name_from_path, face_direction_quat, get_animation_path,
};

use super::components::AnimationEnds;
use super::controller::AnimationController;
use super::data::AnimationData;
use super::human_sequencer::RootMotionMovement;

/// Yaw turn speed of the model while following the movement intent, radians
/// per second.
const TURN_SPEED: f32 = 3.0;

/// Animation graph asset with the human scheme transition graph, generated
/// from the anims archives (see the export tool). Resolved against the crate
/// directory so it is found regardless of the working directory.
const GRAPH_ASSET: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/humans_anim_graph.ron");

/// Additional hand written graph edges, same format as [`GRAPH_ASSET`].
const MANUAL_EDGES_ASSET: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/humans_anim_manual_edges.ron"
);

/// Scheme prefix of the animations driven by this sequencer.
const SCHEME: &str = "HUMANS";

/// The animations the sequencer can desire, resolved to graph nodes.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesiredAnimation {
    /// Standing idle.
    Stand,
    /// Run loop.
    Run,
    /// Bow fire: the reload loop, an arrow shot on every wrap.
    Fire,
}

impl DesiredAnimation {
    /// Graph node name of the animation.
    fn name(self) -> &'static str {
        match self {
            DesiredAnimation::Stand => "S_RUN",
            DesiredAnimation::Run => "S_RUNL",
            // DesiredAnimation::Fire => "S_BOWSHOOT",
            DesiredAnimation::Fire => "T_BOWRELOAD",
        }
    }
}

/// Sets the desired animation of a sequencer entity. Written every frame;
/// the sequencer deduplicates. `direction` is the (horizontal) movement
/// direction the model should face, `None` keeps the last facing.
#[derive(Message, Debug, Clone)]
pub struct SetDesiredAnimation {
    /// Animation model entity carrying the [`AutoAnimSequencer`].
    pub who: Entity,
    pub animation: DesiredAnimation,
    pub force_change: bool,
    pub direction: Option<Vec3>,
}

/// Graph driven animation state of one entity. The desired animation is
/// stored immediately but only applied once the current animation ends.
#[derive(Component, Debug, Default)]
pub struct AutoAnimSequencer {
    /// Desired animation in the graph; `None` keeps the current one.
    pub desired: Option<DesiredAnimation>,
    pub force_change: bool,
}

pub struct AutoAnimSequencerPlugin;

impl Plugin for AutoAnimSequencerPlugin {
    fn build(&self, app: &mut App) {
        match AnimationGraph::load_from_ron(Path::new(GRAPH_ASSET)) {
            Some(mut graph) => {
                graph.load_additional_edges(Path::new(MANUAL_EDGES_ASSET));
                app.insert_resource(graph);
                app.add_systems(Startup, preload_graph_animations);
            }
            None => warn!("AutoAnimSequencerPlugin without an animation graph({GRAPH_ASSET})"),
        }
        app.add_message::<SetDesiredAnimation>();
        app.add_systems(
            Update,
            (
                add_auto_anim_sequencer,
                send_desired_animations,
                run_auto_anim_sequencer.before(super::locomotion::update_animations),
            )
                .chain(),
        );
    }
}

/// Adds the sequencer to the player animation model and marks its physics
/// body as root motion driven. Survives visual respawns.
fn add_auto_anim_sequencer(
    mut commands: Commands,
    mut players: Query<
        (Entity, &mut AnimationController, Option<&ChildOf>),
        (Added<AnimationController>, With<PlayerCharacter>),
    >,
) {
    for (entity, mut controller, parent) in &mut players {
        // The sequencer drives the player by root motion
        controller.set_root_motion(true);
        commands.entity(entity).insert(AutoAnimSequencer::default());
        if let Some(parent) = parent {
            commands.entity(parent.parent()).insert(RootMotionMovement);
        }
    }
}

/// Writes the desired animation of the player every frame from the move
/// input, resolved camera relative (forward input moves away from the
/// camera; holds for every camera mode). Firing (ranged weapon drawn and
/// attack pressed) wins over movement; the model aims at the target.
fn send_desired_animations(
    action_state: Single<&ActionState<GameplayAction>, With<GameplayInputMarker>>,
    players: Query<
        (Entity, Option<&NpcWeaponMode>, &GlobalTransform),
        (With<PlayerCharacter>, With<AutoAnimSequencer>),
    >,
    camera: Single<&Transform, With<Camera3d>>,
    selection: Res<Selection>,
    transforms: Query<&GlobalTransform>,
    mut writer: MessageWriter<SetDesiredAnimation>,
) {
    let Ok((entity, weapon_mode, player_tr)) = players.single() else {
        return;
    };

    let mode = weapon_mode.map_or(WeaponMode::Fists, |mode| mode.0);
    if mode == WeaponMode::Ranged && action_state.pressed(&GameplayAction::Interact) {
        // Aim at the selected npc, fall back to the current facing direction.
        let target = selection
            .target
            .and_then(|target| transforms.get(target).ok())
            .map(|target| target.translation())
            .unwrap_or_else(|| player_tr.translation() + (player_tr.rotation() * Vec3::Z) * 20.0);
        let direction = horizontal(target - player_tr.translation());
        let direction = if direction.length_squared() < 0.0001 {
            None
        } else {
            Some(direction)
        };
        writer.write(SetDesiredAnimation {
            who: entity,
            animation: DesiredAnimation::Fire,
            direction,
            force_change: true,
        });
        return;
    }

    let direction = action_state.axis_pair(&GameplayAction::Move);
    if direction.length_squared() < 0.1 {
        writer.write(SetDesiredAnimation {
            who: entity,
            animation: DesiredAnimation::Stand,
            direction: None,
            force_change: false,
        });
        return;
    }

    let forward = horizontal(*camera.forward());
    let right = horizontal(*camera.right());
    let dir = right * direction.x + forward * direction.y;
    if dir.length_squared() < 0.0001 {
        warn!("Move input above the dead zone but no horizontal camera direction");
        return;
    }
    // println!("SetDesiredAnimation RUN  direction({:?})", direction);
    writer.write(SetDesiredAnimation {
        who: entity,
        animation: DesiredAnimation::Run,
        direction: Some(dir.normalize()),
        force_change: true,
    });
}

/// Direction projected onto the horizontal plane.
fn horizontal(direction: Vec3) -> Vec3 {
    Vec3::new(direction.x, 0.0, direction.z).normalize_or_zero()
}

/// Advances the sequencer: consumes desired animations and animation end
/// events, walks the graph towards the desired animation, rotates the model
/// towards the move direction and keeps the root motion driven body free of
/// horizontal acceleration.
fn run_auto_anim_sequencer(
    time: Res<Time>,
    asset_server: Res<AssetServer>,
    mut handles: ResMut<MaterialHandles>,
    graph: Option<Res<AnimationGraph>>,
    mut desired: MessageReader<SetDesiredAnimation>,
    mut ends: MessageReader<AnimationEnds>,
    mut arrows: MessageWriter<ArrowShot>,
    mut sequencers: Query<(
        Entity,
        &mut AutoAnimSequencer,
        &mut AnimationController,
        &mut Transform,
        Option<&ChildOf>,
    )>,
    mut velocities: Query<
        &mut LinearVelocity,
        (With<PlayerPhysicsBodyMarker>, Without<AutoAnimSequencer>),
    >,
) {
    let Some(graph) = graph else {
        // Every decision needs the graph; without it the sequencer is inert.
        return;
    };
    let graph = &*graph;

    let ends: Vec<_> = ends.read().cloned().collect();
    let desired: Vec<_> = desired.read().cloned().collect();
    let delta = time.delta_secs();

    for (entity, mut seq, mut controller, mut tr, parent) in &mut sequencers {
        for message in desired.iter().filter(|message| message.who == entity) {
            if seq.desired != Some(message.animation) {
                info!(
                    "AutoAnimSequencer({entity}) desired -> {:?}",
                    message.animation
                );
                seq.desired = Some(message.animation);
                seq.force_change = message.force_change;
                let anim_index = index_of(graph, message.animation);
                if seq.force_change {
                    queue_animation(
                        &mut controller,
                        &mut handles,
                        &asset_server,
                        graph.animation_name(anim_index),
                        seq.force_change,
                    );
                }
            }
        }

        // The desired animation is taken into account only after the current
        // animation finishes (its wrap for loops).
        let current_handle = controller.current().clone();
        for _end in ends
            .iter()
            .filter(|end| end.who == entity && end.what_animation == current_handle)
        {
            let Some(current) = animation_index(graph, &current_handle) else {
                warn!(
                    "AutoAnimSequencer({entity}) current animation missing from the graph, desired({:?}) unreachable",
                    seq.desired
                );
                continue;
            };
            let Some(desired) = seq.desired else {
                warn!("No desired");
                continue;
            };
            let desired_index = index_of(graph, desired);
            if current == desired_index {
                if desired == DesiredAnimation::Fire {
                    // The reload loop wrapped: the arrow leaves the bow. The
                    // arrow system aims at the selected target.
                    arrows.write(ArrowShot { shooter: entity });
                }
                // Reached: keep the current animation looping. Held one
                // frame poses emit ends every frame, so this is a normal
                // state, not an anomaly.
                controller.clear_next();
                debug!(
                    "keep looping desired({:?}), current({:?})",
                    desired,
                    controller.current().path()
                );
                continue;
            }
            // Walk the graph; cut straight to the desired animation when the
            // graph offers no path to it.
            let next = graph
                .next_animation(current, desired_index)
                .unwrap_or(desired_index);
            queue_animation(
                &mut controller,
                &mut handles,
                &asset_server,
                graph.animation_name(next),
                seq.force_change,
            );
        }

        // Rotate the model towards the movement direction; standing keeps
        // the last facing.
        if let Some(direction) = desired
            .iter()
            .rev()
            .find(|message| message.who == entity)
            .and_then(|message| message.direction)
        {
            let target = face_direction_quat(direction);
            tr.rotation = tr.rotation.slerp(target, (TURN_SPEED * delta).min(1.0));
        }

        // Root motion drives the horizontal position of the body.
        if let Some(parent) = parent
            && let Ok(mut velocity) = velocities.get_mut(parent.parent())
        {
            velocity.x = 0.0;
            velocity.z = 0.0;
        }
    }
}

/// Preloads every animation of the graph so sequencer switches never have to
/// wait for the animation asset to load. Graph names carry the scheme prefix
/// (`HUMANS-S_RUN`), which maps directly to the asset path scheme.
fn preload_graph_animations(
    graph: Res<AnimationGraph>,
    mut handles: ResMut<MaterialHandles>,
    asset_server: Res<AssetServer>,
) {
    for name in graph.animation_names() {
        let Some((scheme, animation)) = name.split_once('-') else {
            warn!("Animation graph node({name}) without a scheme prefix, it is not preloaded",);
            continue;
        };
        handles.get_animation_handle(&asset_server, &get_animation_path(scheme, animation));
    }
}

/// Graph index of an animation by its sequencer name.
fn index_of(graph: &AnimationGraph, animation: DesiredAnimation) -> AnimationIndex {
    let name = animation.name();
    graph
        .animation_index(&format!("{SCHEME}-{name}"))
        .unwrap_or_else(|| unreachable!("animation({SCHEME}-{name}) missing from the loaded graph"))
}

/// Graph index of the animation a handle refers to, resolved through the
/// asset path (`zengin://.../HUMANS-S_RUN.MAN` -> `HUMANS-S_RUN`).
fn animation_index(
    graph: &AnimationGraph,
    handle: &Handle<AnimationData>,
) -> Option<AnimationIndex> {
    let path = Handle::path(handle)?;
    let name = animation_name_from_path(path.path())?;
    graph.animation_index(&format!("{SCHEME}-{name}"))
}

/// Queues the named graph animation on the controller. Graph names carry the
/// scheme prefix (`HUMANS-T_RUN_2_RUNL`), the asset path adds it itself.
fn queue_animation(
    controller: &mut AnimationController,
    handles: &mut MaterialHandles,
    asset_server: &AssetServer,
    name: &str,
    interrupt_current: bool,
) {
    let animation = name.strip_prefix(&format!("{SCHEME}-")).unwrap_or(name);
    let handle = handles.get_animation_handle(asset_server, &get_animation_path(SCHEME, animation));
    controller.queue_next(handle, interrupt_current);
}

#[cfg(test)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;

    use super::*;

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
        let path = get_animation_path(SCHEME, name);
        world
            .resource_mut::<MaterialHandles>()
            .get_animation_handle(&server, &path)
    }

    fn run_sequencer_world(current_name: &str) -> (World, Entity) {
        let mut world = World::new();
        world.insert_resource(Time::<()>::default());
        world.init_resource::<Messages<AnimationEnds>>();
        world.init_resource::<Messages<SetDesiredAnimation>>();
        world.init_resource::<Messages<ArrowShot>>();
        world.insert_resource(test_asset_server());
        world.insert_resource(MaterialHandles::default());
        let mut graph =
            AnimationGraph::load_from_ron(Path::new(GRAPH_ASSET)).expect("animation graph ron");
        graph.load_additional_edges(Path::new(MANUAL_EDGES_ASSET));
        world.insert_resource(graph);
        let current = animation_handle(&mut world, current_name);
        let model = world
            .spawn((
                GlobalTransform::default(),
                Transform::default(),
                AnimationController::new(current),
                AutoAnimSequencer::default(),
                PlayerCharacter,
            ))
            .id();
        (world, model)
    }

    fn send_desired(world: &mut World, who: Entity, animation: DesiredAnimation) {
        world
            .resource_mut::<Messages<SetDesiredAnimation>>()
            .write(SetDesiredAnimation {
                who,
                animation,
                force_change: false,
                direction: None,
            });
    }

    fn send_ends(world: &mut World, who: Entity, what: Handle<AnimationData>) {
        world
            .resource_mut::<Messages<AnimationEnds>>()
            .write(AnimationEnds {
                who,
                what_animation: what,
            });
    }

    fn queued_next(world: &World, model: Entity) -> Option<Handle<AnimationData>> {
        world
            .get::<AnimationController>(model)
            .unwrap()
            .next()
            .cloned()
    }

    /// Stand wrap with run desired: the graph walk queues the run-in
    /// transition.
    #[test]
    fn stand_wrap_walks_to_run_in() {
        let (mut world, model) = run_sequencer_world("S_RUN");
        let stand = animation_handle(&mut world, "S_RUN");
        let run_in = animation_handle(&mut world, "T_RUN_2_RUNL");

        send_desired(&mut world, model, DesiredAnimation::Run);
        send_ends(&mut world, model, stand);
        world.run_system_once(run_auto_anim_sequencer).unwrap();

        assert_eq!(queued_next(&world, model), Some(run_in));
    }

    /// The run-in finished: the graph walk continues to the run loop.
    #[test]
    fn run_in_end_advances_to_run_loop() {
        let (mut world, model) = run_sequencer_world("T_RUN_2_RUNL");
        let run_in = animation_handle(&mut world, "T_RUN_2_RUNL");
        let run_loop = animation_handle(&mut world, "S_RUNL");

        send_desired(&mut world, model, DesiredAnimation::Run);
        send_ends(&mut world, model, run_in);
        world.run_system_once(run_auto_anim_sequencer).unwrap();

        assert_eq!(queued_next(&world, model), Some(run_loop));
    }

    /// Desired reached on a wrap: the current animation keeps looping.
    #[test]
    fn reached_desired_keeps_looping() {
        let (mut world, model) = run_sequencer_world("S_RUNL");
        let run_loop = animation_handle(&mut world, "S_RUNL");

        send_desired(&mut world, model, DesiredAnimation::Run);
        send_ends(&mut world, model, run_loop.clone());
        world.run_system_once(run_auto_anim_sequencer).unwrap();

        assert_eq!(queued_next(&world, model), None);
    }

    /// Firing: the graph offers no path to the reload loop, so the sequencer
    /// cuts to it; on its wrap the arrow is shot and the loop keeps playing.
    #[test]
    fn firing_cuts_to_reload_and_shoots_on_wrap() {
        let (mut world, model) = run_sequencer_world("S_RUN");
        let stand = animation_handle(&mut world, "S_RUN");
        let reload = animation_handle(&mut world, "T_BOWRELOAD");

        send_desired(&mut world, model, DesiredAnimation::Fire);
        send_ends(&mut world, model, stand);
        world.run_system_once(run_auto_anim_sequencer).unwrap();
        assert_eq!(queued_next(&world, model), Some(reload.clone()));

        // The reload loop wraps: the arrow leaves the bow.
        world
            .get_mut::<AnimationController>(model)
            .unwrap()
            .switch_to(reload.clone());
        send_desired(&mut world, model, DesiredAnimation::Fire);
        send_ends(&mut world, model, reload.clone());
        world.run_system_once(run_auto_anim_sequencer).unwrap();
        assert_eq!(queued_next(&world, model), None);
        assert_eq!(
            world
                .resource::<Messages<ArrowShot>>()
                .iter_current_update_messages()
                .count(),
            1
        );
    }

    /// Releasing the fire button: the reload wrap queues the aim-out
    /// transition instead of shooting, and no arrow is written.
    #[test]
    fn release_fire_leaves_via_aim_out_without_shot() {
        let (mut world, model) = run_sequencer_world("T_BOWRELOAD");
        let reload = animation_handle(&mut world, "T_BOWRELOAD");
        let aim_out = animation_handle(&mut world, "T_BOWAIM_2_BOWRUN");

        send_desired(&mut world, model, DesiredAnimation::Stand);
        send_ends(&mut world, model, reload);
        world.run_system_once(run_auto_anim_sequencer).unwrap();

        assert_eq!(queued_next(&world, model), Some(aim_out));
        assert_eq!(
            world
                .resource::<Messages<ArrowShot>>()
                .iter_current_update_messages()
                .count(),
            0
        );
    }

    /// Stopping: the run loop wrap queues the run-out, its end queues the
    /// standing idle again.
    #[test]
    fn run_loop_stops_via_graph_to_stand() {
        let (mut world, model) = run_sequencer_world("S_RUNL");
        let run_loop = animation_handle(&mut world, "S_RUNL");
        let run_out = animation_handle(&mut world, "T_RUNL_2_RUN");
        let stand = animation_handle(&mut world, "S_RUN");

        send_desired(&mut world, model, DesiredAnimation::Stand);
        send_ends(&mut world, model, run_loop);
        world.run_system_once(run_auto_anim_sequencer).unwrap();
        assert_eq!(queued_next(&world, model), Some(run_out.clone()));

        send_desired(&mut world, model, DesiredAnimation::Stand);
        send_ends(&mut world, model, run_out.clone());
        world
            .get_mut::<AnimationController>(model)
            .unwrap()
            .switch_to(run_out.clone());
        world.run_system_once(run_auto_anim_sequencer).unwrap();
        assert_eq!(queued_next(&world, model), Some(stand));
    }
}
