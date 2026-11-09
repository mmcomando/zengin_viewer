//! Auto animation sequencer: data driven animation selection through the
//! [`AnimationGraph`].
//!
//! Driven by the *desired* animation (arriving as a [`SetDesiredAnimation`]
//! message; other systems may change it): walks the animation graph from the
//! currently played animation towards the desired one. On every
//! [`AnimationEnds`] of the current animation it queries the graph for the
//! next animation on the path and queues it; when the graph offers no path
//! it cuts straight to the desired animation.
//!
//! Movement still works through root motion (`should_move_entity`), the
//! model is rotated smoothly towards the move direction.

use std::path::Path;

use avian3d::prelude::LinearVelocity;
use bevy::prelude::*;

use crate::gameplay::character::PlayerPhysicsBodyMarker;
use crate::gameplay::objects::{NpcModel, PlayerCharacter};
use crate::graphics::{AnimationGraph, AnimationIndex, animation_name_from_path};
use crate::zengin::{
    ArrowShot, CachedAssetHandles, MeleeHit, face_direction_quat, get_animation_path,
};

use super::components::{AnimationEnds, AnimationTrigger};
use super::controller::AnimationController;
use super::data::AnimationData;
use super::send_desired::send_desired_animations;

use crate::game_state::AppState;

pub struct AutoAnimSequencerPlugin;

/// Runs the systems that consume [`SetDesiredAnimation`] and drive the
/// [`AutoAnimSequencer`]. Systems that insert or remove the sequencer have to
/// run before this set so their commands are applied first.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct AutoAnimSequencerSet;

impl Plugin for AutoAnimSequencerPlugin {
    fn build(&self, app: &mut App) {
        if let Some(mut graph) = AnimationGraph::load_from_ron(Path::new(GRAPH_ASSET)) {
            graph.load_additional_edges(Path::new(MANUAL_EDGES_ASSET));
            app.insert_resource(graph);
            app.add_systems(Startup, preload_graph_animations);
        } else {
            warn!("AutoAnimSequencerPlugin without an animation graph({GRAPH_ASSET})");
        }
        app.add_message::<SetDesiredAnimation>();
        app.configure_sets(
            Update,
            AutoAnimSequencerSet.before(super::locomotion::update_animations),
        );
        app.add_systems(
            Update,
            (
                add_auto_anim_sequencer,
                send_desired_animations,
                apply_desired_animations,
                apply_forced_change,
                advance_on_animation_ends,
                update_model_facing,
            )
                .chain()
                .in_set(AutoAnimSequencerSet)
                .run_if(in_state(AppState::InGame)),
        );
        app.add_systems(
            Update,
            emit_melee_hits_on_optimal_frame
                .after(super::locomotion::update_animations)
                .run_if(in_state(AppState::InGame)),
        );
    }
}

/// Yaw turn speed of the model while following the movement intent, radians
/// per second.
const TURN_SPEED: f32 = 6.0;

/// Prefix of the model script event tag marking the impact frame of an attack
/// animation (`OPTIMAL_FRAME` plus the item suffix).
const OPTIMAL_FRAME_TRIGGER: &str = "OPTIMAL_FRAME";

/// Animation graph asset with the human scheme transition graph, generated
/// from the anims archives (see the export tool). Resolved against the crate
/// directory so it is found regardless of the working directory.
const GRAPH_ASSET: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/anim_graph.ron");

/// Additional hand written graph edges, same format as [`GRAPH_ASSET`].
const MANUAL_EDGES_ASSET: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/humans_anim_manual_edges.ron"
);

/// Scheme used for entities whose `NpcModel.animation_scheme` is unknown.
const DEFAULT_SCHEME: &str = "HUMANS";

/// The animations the sequencer can desire, resolved to graph nodes.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesiredAnimation {
    /// Standing idle.
    Stand,
    /// Run loop.
    Run,
    /// Bow fire: the reload loop, an arrow shot on every wrap.
    Fire,
    /// 1H melee attack: the swing, a hit on every loop (wrap). Held while
    /// attacking; the loop makes the swing repeat for the next hit.
    MeleeAttack,
}

impl DesiredAnimation {
    /// Scheme relative animation names this state may resolve to, best match
    /// first. Humans use the `S_RUN`/`T_1HATTACKR` naming, creatures the
    /// `S_FISTRUN`/`S_FISTATTACK` one; a scheme gets its first existing name.
    fn candidates(self) -> &'static [&'static str] {
        match self {
            DesiredAnimation::Stand => &["S_RUN", "S_FISTRUN", "S_FISTRUNL", "S_FISTWALK"],
            DesiredAnimation::Run => &["S_RUNL", "S_FISTRUNL", "S_FISTRUN", "S_FISTWALK"],
            DesiredAnimation::Fire => &["T_BOWRELOAD"],
            DesiredAnimation::MeleeAttack => &["T_1HATTACKR", "S_FISTATTACK", "T_FISTATTACK"],
        }
    }
}

/// Sets the desired animation of a sequencer entity; the sequencer
/// deduplicates. `direction` may also drive the model rotation, `None` keeps
/// the last facing.
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
    /// Direction the model should face; `None` keeps the last facing.
    pub direction: Option<Vec3>,
}

/// Adds the sequencer to the player animation model and enables root motion
/// driven movement. Survives visual respawns.
#[allow(clippy::type_complexity)]
fn add_auto_anim_sequencer(
    mut commands: Commands,
    mut players: Query<
        (Entity, &mut AnimationController),
        (Added<AnimationController>, With<PlayerCharacter>),
    >,
) {
    for (entity, mut controller) in &mut players {
        // The sequencer drives the player by root motion
        controller.set_root_motion(true);
        commands.entity(entity).insert(AutoAnimSequencer::default());
    }
}

/// Consumes [`SetDesiredAnimation`] messages into the [`AutoAnimSequencer`]
/// state; the [`AnimationController`] is only touched by the reaction systems
/// ([`apply_forced_change`] and [`advance_on_animation_ends`]), so a desired
/// animation may lag a frame behind its message.
fn apply_desired_animations(
    mut desired: MessageReader<SetDesiredAnimation>,
    mut sequencers: Query<(&mut AutoAnimSequencer, &AnimationController)>,
) {
    for message in desired.read() {
        let Ok((mut seq, controller)) = sequencers.get_mut(message.who) else {
            warn!(
                "SetDesiredAnimation({:?}) for entity({}) without an AutoAnimSequencer, ignoring",
                message.animation, message.who
            );
            continue;
        };
        if seq.desired == Some(message.animation) {
            // Direction still follows every message; only the animation is
            // deduplicated.
            seq.direction = message.direction;
            continue;
        }
        seq.desired = Some(message.animation);
        seq.force_change = message.force_change;
        // Let a started melee attack finish: interrupting the queued
        // follow-up would cut it mid-way and the hit would never land
        if is_melee_attack(controller.current()) {
            seq.force_change = false;
        }
        seq.direction = message.direction;
    }
}

/// Reacts to [`AutoAnimSequencer`] changes: a forced desired animation cuts
/// to it immediately, interrupting the currently played animation. The
/// non-forced path waits for [`advance_on_animation_ends`].
fn apply_forced_change(
    graph: Option<Res<AnimationGraph>>,
    asset_server: Res<AssetServer>,
    mut handles: ResMut<CachedAssetHandles>,
    mut sequencers: Query<
        (&AutoAnimSequencer, &mut AnimationController, &NpcModel),
        Changed<AutoAnimSequencer>,
    >,
) {
    let Some(graph) = graph else {
        return;
    };
    let graph = graph.into_inner();
    for (seq, mut controller, model) in &mut sequencers {
        let Some(desired) = seq.desired else {
            continue;
        };
        if !seq.force_change {
            continue;
        }
        // A queued hit reaction must not be replaced before it starts.
        if controller.next().is_some_and(is_hit_reaction) {
            continue;
        }
        let scheme = animation_scheme(model);
        let Some(desired) = resolve_animation(graph, scheme, desired) else {
            warn_once!(
                "Scheme({scheme}) has no animation for desired({:?}), it is not queued",
                seq.desired
            );
            continue;
        };
        // A playing hit reaction is queued non-interrupting, the desired
        // animation takes over when the reaction ends.
        let interrupt = !is_hit_reaction(controller.current());
        queue_animation(
            &mut controller,
            &mut handles,
            &asset_server,
            graph.animation_name(desired),
            interrupt,
        );
    }
}

/// Advances the sequencer on [`AnimationEnds`]: walks the graph towards the
/// desired animation and emits the loop wrap side effects (arrow shot).
fn advance_on_animation_ends(
    graph: Option<Res<AnimationGraph>>,
    asset_server: Res<AssetServer>,
    mut handles: ResMut<CachedAssetHandles>,
    mut ends: MessageReader<AnimationEnds>,
    mut arrows: MessageWriter<ArrowShot>,
    mut sequencers: Query<(
        Entity,
        &AutoAnimSequencer,
        &mut AnimationController,
        &NpcModel,
    )>,
) {
    let Some(graph) = graph else {
        // Every decision needs the graph; without it the sequencer is inert.
        return;
    };
    let graph = graph.into_inner();
    for end in ends.read() {
        let Ok((entity, seq, mut controller, model)) = sequencers.get_mut(end.who) else {
            // Most skeletons have no sequencer; their ends are not ours.
            continue;
        };
        if end.what_animation != *controller.current() {
            continue;
        }
        if is_hit_reaction(controller.current()) {
            // The reaction end is handled by `npc_hit_reaction_end`; the
            // reaction is not part of the animation graph.
            continue;
        }
        advance_sequencer_on_end(
            graph,
            entity,
            seq,
            &mut controller,
            animation_scheme(model),
            &mut arrows,
            &mut handles,
            &asset_server,
        );
    }
}

/// Handles one [`AnimationEnds`] for a sequencer-owned skeleton.
#[allow(clippy::too_many_arguments, clippy::cognitive_complexity)]
fn advance_sequencer_on_end(
    graph: &AnimationGraph,
    entity: Entity,
    seq: &AutoAnimSequencer,
    controller: &mut AnimationController,
    scheme: &str,
    arrows: &mut MessageWriter<ArrowShot>,
    handles: &mut CachedAssetHandles,
    asset_server: &AssetServer,
) {
    let Some(current) = animation_index(graph, controller.current()) else {
        warn!(
            "AutoAnimSequencer({entity}) current animation missing from the graph, desired({:?}) unreachable",
            seq.desired
        );
        return;
    };
    let Some(desired) = seq.desired else {
        warn!("AutoAnimSequencer({entity}) animation end without a desired animation");
        return;
    };
    let Some(desired_index) = resolve_animation(graph, scheme, desired) else {
        warn!(
            "AutoAnimSequencer({entity}) scheme({scheme}) has no animation for desired({desired:?})"
        );
        return;
    };
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
        return;
    }
    // A queued hit reaction must not be replaced before it starts.
    if controller.next().is_some_and(is_hit_reaction) {
        return;
    }
    // Walk the graph; cut straight to the desired animation when the
    // graph offers no path to it.
    let next = graph
        .next_animation(current, desired_index)
        .unwrap_or(desired_index);
    queue_animation(
        controller,
        handles,
        asset_server,
        graph.animation_name(next),
        seq.force_change && !is_hit_reaction(controller.current()),
    );
}

/// Emits [`MeleeHit`] when the melee swing animation crosses its optimal
/// frame. The optimal frame is the impact moment of the swing, so the damage
/// lands before the animation ends.
fn emit_melee_hits_on_optimal_frame(
    mut triggers: MessageReader<AnimationTrigger>,
    mut hits: MessageWriter<MeleeHit>,
    sequencers: Query<&AnimationController, With<AutoAnimSequencer>>,
) {
    for trigger in triggers.read() {
        if !trigger.trigger_name.starts_with(OPTIMAL_FRAME_TRIGGER) {
            continue;
        }
        let Ok(controller) = sequencers.get(trigger.who) else {
            continue;
        };
        if !is_melee_attack(controller.current()) {
            continue;
        }
        hits.write(MeleeHit {
            shooter: trigger.who,
        });
    }
}

/// Rotates the model towards the stored movement direction; standing keeps
/// the last facing. Root motion drives the horizontal position of the body,
/// so the velocity it induces on the physics body is zeroed every frame.
fn update_model_facing(
    time: Res<Time>,
    mut sequencers: Query<(&AutoAnimSequencer, &mut Transform, Option<&ChildOf>)>,
    mut velocities: Query<
        &mut LinearVelocity,
        (With<PlayerPhysicsBodyMarker>, Without<AutoAnimSequencer>),
    >,
) {
    let delta = time.delta_secs();
    for (seq, mut tr, parent) in &mut sequencers {
        if let Some(direction) = seq.direction {
            let target = face_direction_quat(direction);
            tr.rotation = tr.rotation.slerp(target, (TURN_SPEED * delta).min(1.0));
        }
        if let Some(parent) = parent
            && let Ok(mut velocity) = velocities.get_mut(parent.parent())
        {
            velocity.x = 0.0;
            velocity.z = 0.0;
        }
    }
}

fn preload_graph_animations(
    graph: Res<AnimationGraph>,
    mut handles: ResMut<CachedAssetHandles>,
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

/// Graph index of the animation a handle refers to, resolved through the
/// asset path (`zengin://.../HUMANS-S_RUN.MAN` -> `HUMANS-S_RUN`).
fn animation_index(
    graph: &AnimationGraph,
    handle: &Handle<AnimationData>,
) -> Option<AnimationIndex> {
    let path = Handle::path(handle)?;
    let name = path.path().file_name()?.to_str()?.strip_suffix(".MAN")?;
    graph.animation_index(name)
}

/// Animation scheme of the sequencer entity, `HUMANS` when unknown.
fn animation_scheme(model: &NpcModel) -> &str {
    model.animation_scheme.as_deref().unwrap_or(DEFAULT_SCHEME)
}

/// Graph node of the desired animation in the scheme, the first existing
/// candidate wins. `None` when the scheme has none of them.
fn resolve_animation(
    graph: &AnimationGraph,
    scheme: &str,
    desired: DesiredAnimation,
) -> Option<AnimationIndex> {
    desired
        .candidates()
        .iter()
        .find_map(|animation| graph.animation_index(&format!("{scheme}-{animation}")))
}

/// Animation name part of a handle (`zengin://.../WOLF-S_FISTATTACK.MAN` ->
/// `S_FISTATTACK`).
fn animation_name(handle: &Handle<AnimationData>) -> Option<&str> {
    let path = Handle::path(handle)?;
    animation_name_from_path(path.path())
}

/// Whether the animation handle is one of the melee attack animations.
fn is_melee_attack(handle: &Handle<AnimationData>) -> bool {
    animation_name(handle)
        .is_some_and(|name| DesiredAnimation::MeleeAttack.candidates().contains(&name))
}

/// Whether the animation handle is the `T_GOTHIT` hit reaction one-shot.
fn is_hit_reaction(handle: &Handle<AnimationData>) -> bool {
    animation_name(handle) == Some("T_GOTHIT")
}

/// Queues the named graph animation on the controller. Graph names carry the
/// scheme prefix (`HUMANS-T_RUN_2_RUNL`, `WOLF-S_FISTRUNL`), the asset path
/// adds it itself.
fn queue_animation(
    controller: &mut AnimationController,
    handles: &mut CachedAssetHandles,
    asset_server: &AssetServer,
    name: &str,
    interrupt_current: bool,
) {
    let (scheme, animation) = name.split_once('-').unwrap_or((DEFAULT_SCHEME, name));
    let handle = handles.get_animation_handle(asset_server, &get_animation_path(scheme, animation));
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
        animation_handle_for(world, DEFAULT_SCHEME, name)
    }

    fn animation_handle_for(world: &mut World, scheme: &str, name: &str) -> Handle<AnimationData> {
        let server = world.resource::<AssetServer>().clone();
        let path = get_animation_path(scheme, name);
        world
            .resource_mut::<CachedAssetHandles>()
            .get_animation_handle(&server, &path)
    }

    fn run_sequencer_world(current_name: &str) -> (World, Entity) {
        run_sequencer_world_scheme(current_name, DEFAULT_SCHEME)
    }

    fn run_sequencer_world_scheme(current_name: &str, scheme: &str) -> (World, Entity) {
        let mut world = World::new();
        world.insert_resource(Time::<()>::default());
        world.init_resource::<Messages<AnimationEnds>>();
        world.init_resource::<Messages<AnimationTrigger>>();
        world.init_resource::<Messages<SetDesiredAnimation>>();
        world.init_resource::<Messages<ArrowShot>>();
        world.init_resource::<Messages<MeleeHit>>();
        world.insert_resource(test_asset_server());
        world.insert_resource(CachedAssetHandles::default());
        let mut graph =
            AnimationGraph::load_from_ron(Path::new(GRAPH_ASSET)).expect("animation graph ron");
        graph.load_additional_edges(Path::new(MANUAL_EDGES_ASSET));
        world.insert_resource(graph);
        let current = animation_handle_for(&mut world, scheme, current_name);
        let model = world
            .spawn((
                GlobalTransform::default(),
                Transform::default(),
                AnimationController::new(current),
                AutoAnimSequencer::default(),
                NpcModel {
                    animation_scheme: Some(scheme.to_string()),
                    ..default()
                },
                PlayerCharacter,
            ))
            .id();
        (world, model)
    }

    fn send_desired(world: &mut World, who: Entity, animation: DesiredAnimation) {
        send_desired_force(world, who, animation, false);
    }

    fn send_desired_force(
        world: &mut World,
        who: Entity,
        animation: DesiredAnimation,
        force_change: bool,
    ) {
        world
            .resource_mut::<Messages<SetDesiredAnimation>>()
            .write(SetDesiredAnimation {
                who,
                animation,
                force_change,
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

    fn send_optimal_frame(world: &mut World, who: Entity) {
        world
            .resource_mut::<Messages<AnimationTrigger>>()
            .write(AnimationTrigger {
                who,
                trigger_name: "OPTIMAL_FRAME_".to_string(),
            });
    }

    fn run_optimal_frame_system(world: &mut World) {
        world
            .run_system_once(emit_melee_hits_on_optimal_frame)
            .unwrap();
    }

    fn melee_hits(world: &World) -> usize {
        world
            .resource::<Messages<MeleeHit>>()
            .iter_current_update_messages()
            .count()
    }

    /// Runs the sequencer reaction systems in the plugin order; the facing
    /// system is left out as the tests send no directions.
    fn run_sequencer(world: &mut World) {
        world.run_system_once(apply_desired_animations).unwrap();
        world.run_system_once(apply_forced_change).unwrap();
        world.run_system_once(advance_on_animation_ends).unwrap();
    }

    fn queued_next(world: &World, model: Entity) -> Option<Handle<AnimationData>> {
        world.get::<AnimationController>(model)?.next().cloned()
    }

    #[test]
    fn stand_wrap_walks_to_run_in() {
        let (mut world, model) = run_sequencer_world("S_RUN");
        let stand = animation_handle(&mut world, "S_RUN");
        let run_in = animation_handle(&mut world, "T_RUN_2_RUNL");

        send_desired(&mut world, model, DesiredAnimation::Run);
        send_ends(&mut world, model, stand);
        run_sequencer(&mut world);

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
        run_sequencer(&mut world);

        assert_eq!(queued_next(&world, model), Some(run_loop));
    }

    /// Desired reached on a wrap: the current animation keeps looping.
    #[test]
    fn reached_desired_keeps_looping() {
        let (mut world, model) = run_sequencer_world("S_RUNL");
        let run_loop = animation_handle(&mut world, "S_RUNL");

        send_desired(&mut world, model, DesiredAnimation::Run);
        send_ends(&mut world, model, run_loop.clone());
        run_sequencer(&mut world);

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
        run_sequencer(&mut world);
        assert_eq!(queued_next(&world, model), Some(reload.clone()));

        // The reload loop wraps: the arrow leaves the bow.
        world
            .get_mut::<AnimationController>(model)
            .unwrap()
            .switch_to(reload.clone());
        send_desired(&mut world, model, DesiredAnimation::Fire);
        send_ends(&mut world, model, reload.clone());
        run_sequencer(&mut world);
        assert_eq!(queued_next(&world, model), None);
        assert_eq!(
            world
                .resource::<Messages<ArrowShot>>()
                .iter_current_update_messages()
                .count(),
            1
        );
    }

    #[test]
    fn release_fire_walks_to_shoot_without_shot() {
        let (mut world, model) = run_sequencer_world("T_BOWRELOAD");
        let reload = animation_handle(&mut world, "T_BOWRELOAD");
        let shoot = animation_handle(&mut world, "S_BOWSHOOT");

        send_desired(&mut world, model, DesiredAnimation::Stand);
        send_ends(&mut world, model, reload);
        run_sequencer(&mut world);

        assert_eq!(queued_next(&world, model), Some(shoot));
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
        run_sequencer(&mut world);
        assert_eq!(queued_next(&world, model), Some(run_out.clone()));

        send_desired(&mut world, model, DesiredAnimation::Stand);
        send_ends(&mut world, model, run_out.clone());
        world
            .get_mut::<AnimationController>(model)
            .unwrap()
            .switch_to(run_out.clone());
        run_sequencer(&mut world);
        assert_eq!(queued_next(&world, model), Some(stand));
    }

    /// Melee attack: crossing the optimal frame lands the `MeleeHit`; the
    /// swing keeps looping while the attack is held.
    #[test]
    fn melee_swing_hits_on_optimal_frame() {
        let (mut world, model) = run_sequencer_world("T_1HATTACKR");
        let swing = animation_handle(&mut world, "T_1HATTACKR");

        send_desired(&mut world, model, DesiredAnimation::MeleeAttack);
        send_ends(&mut world, model, swing);
        run_sequencer(&mut world);
        // Reached: the swing keeps looping, no animation queued
        assert_eq!(queued_next(&world, model), None);

        send_optimal_frame(&mut world, model);
        run_optimal_frame_system(&mut world);
        assert_eq!(melee_hits(&world), 1);
    }

    /// Releasing the attack mid-swing still lands the hit on the optimal
    /// frame; only the animation end queues the follow-up (here the standing
    /// idle).
    #[test]
    fn melee_hit_lands_when_attack_released_mid_swing() {
        let (mut world, model) = run_sequencer_world("T_1HATTACKR");
        let swing = animation_handle(&mut world, "T_1HATTACKR");
        let stand = animation_handle(&mut world, "S_RUN");

        // Attack starts (the real input path sends force_change)
        send_desired_force(&mut world, model, DesiredAnimation::MeleeAttack, true);
        // Released before the swing ended
        send_desired(&mut world, model, DesiredAnimation::Stand);
        run_sequencer(&mut world);

        // The optimal frame still lands the hit.
        send_optimal_frame(&mut world, model);
        run_optimal_frame_system(&mut world);
        assert_eq!(melee_hits(&world), 1);

        // The swing end queues the follow-up stand afterwards.
        send_ends(&mut world, model, swing);
        run_sequencer(&mut world);
        assert_eq!(queued_next(&world, model), Some(stand));
    }

    /// A queued hit reaction is not replaced by a forced desired animation
    /// before it starts.
    #[test]
    fn queued_hit_reaction_is_not_replaced() {
        let (mut world, model) = run_sequencer_world("S_RUN");
        let reaction = animation_handle(&mut world, "T_GOTHIT");
        world
            .get_mut::<AnimationController>(model)
            .unwrap()
            .queue_next(reaction.clone(), true);

        send_desired_force(&mut world, model, DesiredAnimation::Run, true);
        run_sequencer(&mut world);

        assert_eq!(queued_next(&world, model), Some(reaction));
    }

    /// A playing hit reaction is not interrupted by a forced desired
    /// animation; the desired one is queued behind it.
    #[test]
    fn playing_hit_reaction_is_not_interrupted() {
        let (mut world, model) = run_sequencer_world("S_RUN");
        let reaction = animation_handle(&mut world, "T_GOTHIT");
        let run_loop = animation_handle(&mut world, "S_RUNL");
        world
            .get_mut::<AnimationController>(model)
            .unwrap()
            .switch_to(reaction);

        send_desired_force(&mut world, model, DesiredAnimation::Run, true);
        run_sequencer(&mut world);

        let controller = world.get::<AnimationController>(model).unwrap();
        assert_eq!(controller.next(), Some(&run_loop));
        assert!(!controller.interrupt_current());
    }

    /// A creature scheme resolves `MeleeAttack` to its `S_FISTATTACK` state.
    #[test]
    fn wolf_melee_attack_uses_fist_attack() {
        let (mut world, model) = run_sequencer_world_scheme("S_FISTRUN", "WOLF");
        let attack = animation_handle_for(&mut world, "WOLF", "S_FISTATTACK");

        send_desired_force(&mut world, model, DesiredAnimation::MeleeAttack, true);
        run_sequencer(&mut world);

        assert_eq!(queued_next(&world, model), Some(attack));
    }

    /// A creature scheme resolves Stand to its `S_FISTRUN` idle.
    #[test]
    fn wolf_stand_uses_fist_run() {
        let (mut world, model) = run_sequencer_world_scheme("S_FISTRUNL", "WOLF");
        let stand = animation_handle_for(&mut world, "WOLF", "S_FISTRUN");

        send_desired_force(&mut world, model, DesiredAnimation::Stand, true);
        run_sequencer(&mut world);

        assert_eq!(queued_next(&world, model), Some(stand));
    }

    /// Schemes without an `S_FISTRUN` (ALLIGATOR) fall back to the nearest
    /// existing candidate, `S_FISTRUNL`.
    #[test]
    fn alligator_stand_falls_back_to_fist_run_loop() {
        let (mut world, model) = run_sequencer_world_scheme("S_FISTWALK", "ALLIGATOR");
        let stand = animation_handle_for(&mut world, "ALLIGATOR", "S_FISTRUNL");

        send_desired_force(&mut world, model, DesiredAnimation::Stand, true);
        run_sequencer(&mut world);

        assert_eq!(queued_next(&world, model), Some(stand));
    }
}
