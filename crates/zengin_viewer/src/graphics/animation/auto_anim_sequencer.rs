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

use crate::gameplay::objects::{NpcModel, PlayerCharacter};
use crate::graphics::{AnimationGraph, AnimationIndex, animation_name_from_path};
use crate::zengin::{
    ArrowShot, CachedAssetHandles, MeleeHit, NpcWeaponMode, WeaponMode, face_direction_quat,
    get_animation_path,
};

use super::components::{AnimationEnds, AnimationTrigger};
use super::controller::AnimationController;
use super::data::AnimationData;
use super::send_desired::send_desired_animations;

use crate::game_state::AppState;
use crate::input::world_input_enabled;

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
                enable_player_root_motion,
                send_desired_animations.run_if(world_input_enabled),
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

/// Yaw turn speed while turning in place with fists, radians per second
/// (Gothic's 90 degrees).
const TURN_IN_PLACE_SPEED_FISTS: f32 = std::f32::consts::FRAC_PI_2;

/// Yaw turn speed while turning in place with a weapon drawn, radians per
/// second (Gothic's 180 degrees).
const TURN_IN_PLACE_SPEED_ARMED: f32 = std::f32::consts::PI;

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
    /// Walk loop (`AI_SetWalkMode` with `NPC_WALK`).
    Walk,
    /// Sneak loop (`AI_SetWalkMode` with `NPC_SNEAK`).
    Sneak,
    /// Bow fire: the reload loop, an arrow shot on every wrap.
    Fire,
    /// Melee attack: the swing, a hit on every loop (wrap). Held while
    /// attacking; the loop makes the swing repeat for the next hit.
    MeleeAttack(MeleeAttackKind),
    /// Turn in place towards the camera direction.
    Turn(TurnDirection),
    /// Death: the final lying pose, reached through the scheme's death
    /// transition when one exists.
    Dead,
    /// Sideways run loop that keeps the facing.
    StrafeLeft,
    /// Sideways run loop that keeps the facing.
    StrafeRight,
}

/// Which way the model turns in place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnDirection {
    Left,
    Right,
}

impl TurnDirection {
    /// Scheme relative turn animation names, best match first.
    fn candidates(self) -> &'static [&'static str] {
        match self {
            Self::Left => &["T_RUNTURNL", "T_FISTRUNTURNL"],
            Self::Right => &["T_RUNTURNR", "T_FISTRUNTURNR"],
        }
    }
}

/// Which melee attack animation to play; the animation scheme and weapon
/// class resolve the concrete animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeleeAttackKind {
    /// The weapon's default swing; for 1H weapons it is the right swing.
    Default,
    /// Left swing.
    Left,
    /// Right swing, the same animation as [`Self::Default`] for 1H weapons.
    Right,
    /// Unarmed and claw attack, for schemes without weapon swings.
    Fist,
}

impl MeleeAttackKind {
    /// Every attack kind, used to recognize attack animations.
    const ALL: [Self; 4] = [Self::Default, Self::Left, Self::Right, Self::Fist];

    /// Scheme relative animation names this kind may resolve to, best match
    /// first; a scheme gets its first existing name. 1H schemes only have
    /// left/right swings, so the default one is the right swing.
    fn candidates(self) -> &'static [&'static str] {
        match self {
            Self::Default | Self::Right => &["T_1HATTACKR", "S_FISTATTACK", "T_FISTATTACK"],
            Self::Left => &["T_1HATTACKL", "S_FISTATTACK", "T_FISTATTACK"],
            Self::Fist => &["S_FISTATTACK", "T_FISTATTACK"],
        }
    }
}

impl DesiredAnimation {
    /// Scheme relative animation names this state may resolve to, best match
    /// first. Humans use the `S_RUN`/`T_1HATTACKR` naming, creatures the
    /// `S_FISTRUN`/`S_FISTATTACK` one; a scheme gets its first existing name.
    fn candidates(self, weapon_mode: WeaponMode) -> &'static [&'static str] {
        match self {
            DesiredAnimation::Stand => &["S_RUN", "S_FISTRUN", "S_FISTRUNL", "S_FISTWALK"],
            DesiredAnimation::Run => &["S_RUNL", "S_FISTRUNL", "S_FISTRUN", "S_FISTWALK"],
            DesiredAnimation::Walk => walk_candidates(weapon_mode),
            DesiredAnimation::Sneak => sneak_candidates(weapon_mode),
            DesiredAnimation::Fire => &["T_BOWRELOAD"],
            DesiredAnimation::MeleeAttack(kind) => kind.candidates(),
            DesiredAnimation::Turn(direction) => direction.candidates(),
            DesiredAnimation::Dead => &["S_DEAD", "S_DEADB"],
            DesiredAnimation::StrafeLeft => strafe_candidates("L", weapon_mode),
            DesiredAnimation::StrafeRight => strafe_candidates("R", weapon_mode),
        }
    }
}

/// Walk loop candidates for the drawn weapon, then the unarmed and fist
/// fallbacks (humans use `S_WALKL`, creatures `S_FISTWALKL`).
fn walk_candidates(weapon_mode: WeaponMode) -> &'static [&'static str] {
    match weapon_mode {
        WeaponMode::Melee => &["S_1HWALKL", "S_2HWALKL", "S_WALKL", "S_FISTWALKL"],
        WeaponMode::Ranged => &["S_BOWWALKL", "S_CBOWWALKL", "S_WALKL", "S_FISTWALKL"],
        WeaponMode::Fists => &["S_WALKL", "S_FISTWALKL"],
    }
}

/// Sneak loop candidates for the drawn weapon, then the unarmed and fist
/// fallbacks (humans use `S_SNEAKL`, creatures `S_FISTSNEAKL`).
fn sneak_candidates(weapon_mode: WeaponMode) -> &'static [&'static str] {
    match weapon_mode {
        WeaponMode::Melee => &["S_1HSNEAKL", "S_2HSNEAKL", "S_SNEAKL", "S_FISTSNEAKL"],
        WeaponMode::Ranged => &["S_BOWSNEAKL", "S_CBOWSNEAKL", "S_SNEAKL", "S_FISTSNEAKL"],
        WeaponMode::Fists => &["S_SNEAKL", "S_FISTSNEAKL"],
    }
}

/// Strafe loop candidates for the drawn weapon, then the unarmed and fist
/// fallbacks. `side` is the animation name suffix (`L` or `R`).
fn strafe_candidates(side: &str, weapon_mode: WeaponMode) -> &'static [&'static str] {
    match (side, weapon_mode) {
        ("L", WeaponMode::Melee) => &["T_1HRUNSTRAFEL", "T_RUNSTRAFEL", "T_FISTRUNSTRAFEL"],
        ("L", WeaponMode::Ranged) => &[
            "T_BOWRUNSTRAFEL",
            "T_CBOWRUNSTRAFEL",
            "T_RUNSTRAFEL",
            "T_FISTRUNSTRAFEL",
        ],
        ("L", WeaponMode::Fists) => &["T_FISTRUNSTRAFEL", "T_RUNSTRAFEL"],
        (_, WeaponMode::Melee) => &["T_1HRUNSTRAFER", "T_RUNSTRAFER", "T_FISTRUNSTRAFER"],
        (_, WeaponMode::Ranged) => &[
            "T_BOWRUNSTRAFER",
            "T_CBOWRUNSTRAFER",
            "T_RUNSTRAFER",
            "T_FISTRUNSTRAFER",
        ],
        (_, WeaponMode::Fists) => &["T_FISTRUNSTRAFER", "T_RUNSTRAFER"],
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

impl AutoAnimSequencer {
    /// Sets the desired animation and switches to it immediately, cutting the
    /// current animation even when it is a started melee swing.
    pub fn set_desired_now(&mut self, animation: DesiredAnimation) {
        self.desired = Some(animation);
        self.force_change = true;
    }

    /// Stops the graph from driving the animation. A directly queued animation
    /// (scripted, item or mob pose) is left to play and loop on its own.
    pub fn suspend(&mut self) {
        self.desired = None;
        self.force_change = false;
        self.direction = None;
    }
}

/// Enables root motion driven movement of the player visual. The sequencer is
/// inserted together with the [`AnimationController`] by the near-distance npc
/// spawn; this only has to survive visual respawns, which re-insert the
/// controller.
fn enable_player_root_motion(
    mut players: Query<
        &mut AnimationController,
        (Added<AnimationController>, With<PlayerCharacter>),
    >,
) {
    for mut controller in &mut players {
        controller.set_root_motion(true);
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
        if is_melee_attack_animation(controller.current()) {
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
        (
            &AutoAnimSequencer,
            &mut AnimationController,
            &NpcModel,
            Option<&NpcWeaponMode>,
        ),
        Changed<AutoAnimSequencer>,
    >,
) {
    let Some(graph) = graph else {
        return;
    };
    let graph = graph.into_inner();
    for (seq, mut controller, model, weapon_mode) in &mut sequencers {
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
        let Some(desired) = resolve_animation(graph, scheme, desired, weapon_mode) else {
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
        Option<&NpcWeaponMode>,
    )>,
) {
    let Some(graph) = graph else {
        // Every decision needs the graph; without it the sequencer is inert.
        return;
    };
    let graph = graph.into_inner();
    for end in ends.read() {
        let Ok((entity, seq, mut controller, model, weapon_mode)) = sequencers.get_mut(end.who)
        else {
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
            weapon_mode,
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
    weapon_mode: Option<&NpcWeaponMode>,
    arrows: &mut MessageWriter<ArrowShot>,
    handles: &mut CachedAssetHandles,
    asset_server: &AssetServer,
) {
    let Some(desired) = seq.desired else {
        // Passive sequencer: the animation is driven directly (scripted, item
        // or mob pose), leave it alone.
        return;
    };
    let Some(desired_index) = resolve_animation(graph, scheme, desired, weapon_mode) else {
        warn!(
            "AutoAnimSequencer({entity}) scheme({scheme}) has no animation for desired({desired:?})"
        );
        return;
    };
    let Some(current) = animation_index(graph, controller.current()) else {
        // The current animation is not a graph node (a pose of another scheme),
        // so there is no path to walk: cut straight to the desired one.
        debug!(
            "AutoAnimSequencer({entity}) current animation not in the graph, cutting to desired({desired:?})"
        );
        queue_animation(
            controller,
            handles,
            asset_server,
            graph.animation_name(desired_index),
            seq.force_change && !is_hit_reaction(controller.current()),
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
        if !is_melee_attack_animation(controller.current()) {
            continue;
        }
        hits.write(MeleeHit {
            shooter: trigger.who,
        });
    }
}

/// Rotates the entity towards its stored movement direction. The capsule is
/// symmetric, so driving the physics body's yaw for facing is safe.
fn update_model_facing(
    time: Res<Time>,
    mut sequencers: Query<(
        &AutoAnimSequencer,
        &mut Transform,
        Option<&NpcWeaponMode>,
        Option<&mut LinearVelocity>,
    )>,
) {
    let delta = time.delta_secs();
    for (seq, mut tr, weapon_mode, velocity) in &mut sequencers {
        if let Some(direction) = seq.direction {
            let target = face_direction_quat(direction);
            let speed = model_turn_speed(seq, weapon_mode);
            tr.rotation = tr.rotation.slerp(target, (speed * delta).min(1.0));
        }
        if let Some(mut velocity) = velocity {
            velocity.x = 0.0;
            velocity.z = 0.0;
        }
    }
}

/// Yaw turn speed of the model: idle turning follows the camera slowly, the
/// movement intent turns fast.
fn model_turn_speed(seq: &AutoAnimSequencer, weapon_mode: Option<&NpcWeaponMode>) -> f32 {
    match seq.desired {
        Some(DesiredAnimation::Stand | DesiredAnimation::Turn(_)) => {
            match weapon_mode.map(|mode| mode.0) {
                Some(WeaponMode::Melee | WeaponMode::Ranged) => TURN_IN_PLACE_SPEED_ARMED,
                Some(WeaponMode::Fists) | None => TURN_IN_PLACE_SPEED_FISTS,
            }
        }
        Some(
            DesiredAnimation::Run
            | DesiredAnimation::Walk
            | DesiredAnimation::Sneak
            | DesiredAnimation::Fire
            | DesiredAnimation::Dead
            | DesiredAnimation::MeleeAttack(_)
            | DesiredAnimation::StrafeLeft
            | DesiredAnimation::StrafeRight,
        )
        | None => TURN_SPEED,
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
    weapon_mode: Option<&NpcWeaponMode>,
) -> Option<AnimationIndex> {
    let weapon_mode = weapon_mode.map_or(WeaponMode::Fists, |mode| mode.0);
    desired
        .candidates(weapon_mode)
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
pub(crate) fn is_melee_attack_animation(handle: &Handle<AnimationData>) -> bool {
    animation_name(handle).is_some_and(|name| {
        MeleeAttackKind::ALL
            .iter()
            .any(|kind| kind.candidates().contains(&name))
    })
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
    fn turn_left_cuts_to_left_turn_animation() {
        let (mut world, model) = run_sequencer_world("S_RUN");
        let turn = animation_handle(&mut world, "T_RUNTURNL");

        send_desired_force(
            &mut world,
            model,
            DesiredAnimation::Turn(TurnDirection::Left),
            true,
        );
        run_sequencer(&mut world);

        assert_eq!(queued_next(&world, model), Some(turn));
    }

    #[test]
    fn turn_right_cuts_to_right_turn_animation() {
        let (mut world, model) = run_sequencer_world("S_RUN");
        let turn = animation_handle(&mut world, "T_RUNTURNR");

        send_desired_force(
            &mut world,
            model,
            DesiredAnimation::Turn(TurnDirection::Right),
            true,
        );
        run_sequencer(&mut world);

        assert_eq!(queued_next(&world, model), Some(turn));
    }

    /// The turn animation loops while the camera keeps turning; releasing it
    /// returns to the standing idle.
    #[test]
    fn released_turn_returns_to_stand() {
        let (mut world, model) = run_sequencer_world("T_RUNTURNL");
        let turn = animation_handle(&mut world, "T_RUNTURNL");
        let stand = animation_handle(&mut world, "S_RUN");

        send_desired(
            &mut world,
            model,
            DesiredAnimation::Turn(TurnDirection::Left),
        );
        send_ends(&mut world, model, turn.clone());
        run_sequencer(&mut world);
        assert_eq!(queued_next(&world, model), None);

        send_desired(&mut world, model, DesiredAnimation::Stand);
        send_ends(&mut world, model, turn);
        run_sequencer(&mut world);
        assert_eq!(queued_next(&world, model), Some(stand));
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
    fn strafe_left_uses_the_unarmed_strafe_loop() {
        let (mut world, model) = run_sequencer_world("S_RUN");
        let strafe = animation_handle(&mut world, "T_FISTRUNSTRAFEL");

        send_desired_force(&mut world, model, DesiredAnimation::StrafeLeft, true);
        run_sequencer(&mut world);

        assert_eq!(queued_next(&world, model), Some(strafe));
    }

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

        send_desired(
            &mut world,
            model,
            DesiredAnimation::MeleeAttack(MeleeAttackKind::Default),
        );
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
        send_desired_force(
            &mut world,
            model,
            DesiredAnimation::MeleeAttack(MeleeAttackKind::Default),
            true,
        );
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

    /// A directional press plays exactly one swing: the forced start cuts into
    /// the swing, the next frame's stand replaces the desired animation, the
    /// optimal frame lands the hit and the swing end queues the stand.
    #[test]
    fn directional_swing_is_one_shot() {
        let (mut world, model) = run_sequencer_world("S_RUN");
        let attack = animation_handle(&mut world, "T_1HATTACKL");
        let stand = animation_handle(&mut world, "S_RUN");

        send_desired_force(
            &mut world,
            model,
            DesiredAnimation::MeleeAttack(MeleeAttackKind::Left),
            true,
        );
        run_sequencer(&mut world);
        assert_eq!(queued_next(&world, model), Some(attack.clone()));

        // The animation update switched to the swing before the next frame.
        world
            .get_mut::<AnimationController>(model)
            .unwrap()
            .switch_to(attack.clone());
        send_desired(&mut world, model, DesiredAnimation::Stand);
        run_sequencer(&mut world);

        send_optimal_frame(&mut world, model);
        run_optimal_frame_system(&mut world);
        assert_eq!(melee_hits(&world), 1);

        send_ends(&mut world, model, attack);
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

        send_desired_force(
            &mut world,
            model,
            DesiredAnimation::MeleeAttack(MeleeAttackKind::Default),
            true,
        );
        run_sequencer(&mut world);

        assert_eq!(queued_next(&world, model), Some(attack));
    }

    #[test]
    fn left_swing_uses_the_left_attack_animation() {
        let (mut world, model) = run_sequencer_world("S_RUN");
        let attack = animation_handle(&mut world, "T_1HATTACKL");

        send_desired_force(
            &mut world,
            model,
            DesiredAnimation::MeleeAttack(MeleeAttackKind::Left),
            true,
        );
        run_sequencer(&mut world);

        assert_eq!(queued_next(&world, model), Some(attack));
    }

    #[test]
    fn fist_kind_uses_the_fist_attack_animation() {
        let (mut world, model) = run_sequencer_world("S_RUN");
        let attack = animation_handle(&mut world, "S_FISTATTACK");

        send_desired_force(
            &mut world,
            model,
            DesiredAnimation::MeleeAttack(MeleeAttackKind::Fist),
            true,
        );
        run_sequencer(&mut world);

        assert_eq!(queued_next(&world, model), Some(attack));
    }

    #[test]
    fn left_swing_lands_a_melee_hit_on_the_optimal_frame() {
        let (mut world, model) = run_sequencer_world("T_1HATTACKL");

        send_optimal_frame(&mut world, model);
        run_optimal_frame_system(&mut world);

        assert_eq!(melee_hits(&world), 1);
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
