use std::collections::HashSet;

use bevy::prelude::*;

use crate::gameplay::objects::{NpcBodyRootOffset, NpcModel, PlayerCharacter};
use crate::graphics::{
    AnimationController, AnimationEnds, AutoAnimSequencer, DesiredAnimation, SetDesiredAnimation,
    animation_name_from_path,
};
use crate::zengin::common::{
    MAX_AI_USE_DISTANCE, MOB_SEARCH_RADIUS, face_direction_quat, horizontal_offset,
};
use crate::zengin::npc_actions::action::ExecutingActionMarker;
use crate::zengin::npc_routines::{
    AnimationFiles, NPC_BODY_Y_OFFSET, NpcOverlay, has_animation, queue_named_animation,
    queue_root_motion_animation, queue_stand_animation,
};
use crate::zengin::resources::{CachedAssetHandles, ZenGinModelComponent};
use crate::zengin::script::component::ScriptResource;
use crate::zengin::script::string_value::StringCanon;
use crate::zengin::{GameMob, GameNpc, MobBase, MobOccupant, MobPose, MobRegistry, NpcMobUse};

const MOB_USE_DISTANCE: f32 = MAX_AI_USE_DISTANCE;

/// Uses a mob with the given `AI_UseMob` scheme.
#[derive(Component)]
#[require(ExecutingActionMarker)]
pub(crate) struct UseMobAction {
    pub(crate) scheme: StringCanon,
    pub(crate) state: i32,
    /// Use exactly this mob instead of the nearest one of `scheme`.
    pub(crate) target: Option<Entity>,
    /// Snap to the attachment point instead of walking to it.
    pub(crate) snap: bool,
    /// Player switch use (lever/button/wheel/door): pose and mob state to apply.
    /// `state` stays positive so this is not treated as a detach request.
    pub(crate) switch_state: Option<i32>,
}

impl UseMobAction {
    /// `AI_UseMob` request of an npc: nearest free mob of the scheme.
    pub(crate) fn npc(scheme: StringCanon, state: i32) -> Self {
        Self {
            scheme,
            state,
            target: None,
            snap: false,
            switch_state: None,
        }
    }

    /// Player request for a selected mob.
    pub(crate) fn player(target: Entity, scheme: StringCanon, state: i32) -> Self {
        Self {
            scheme,
            state,
            target: Some(target),
            snap: true,
            switch_state: None,
        }
    }

    /// Player use of a switch-like mob: `state` is the toggled state (0 or 1).
    pub(crate) fn player_switch(target: Entity, scheme: StringCanon, state: i32) -> Self {
        Self {
            scheme,
            state: state.max(1),
            target: Some(target),
            snap: true,
            switch_state: Some(state),
        }
    }
}

/// Pose queued as a follow-up of the entering transition.
///
/// The engine enters a mob through `T_<SCHEME>[_<POS>]_S<from>_2_S<to>` and only
/// then holds the `S_<SCHEME>[_<POS>]_S<n>` pose. Some poses (e.g.
/// `S_BEDHIGH_FRONT_S1`) do not animate the base bone at all, so playing them
/// directly leaves the npc standing upright: they inherit the root transform
/// from the transition.
#[derive(Component, Debug)]
pub(crate) struct MobUsePose {
    /// Entering transition whose end starts the pose.
    transition: String,
    /// Pose held once the transition ends.
    pose: String,
}

/// Queues the mob pose when the entering transition ends.
pub(crate) fn advance_mob_use_pose(
    mut commands: Commands,
    mut ends: MessageReader<AnimationEnds>,
    files: Res<AnimationFiles>,
    mut handles: ResMut<CachedAssetHandles>,
    asset_server: Res<AssetServer>,
    mut npcs: Query<(
        &mut AnimationController,
        &NpcModel,
        Option<&NpcOverlay>,
        &MobUsePose,
    )>,
    mut released: RemovedComponents<NpcMobUse>,
) {
    // A use that ends before the transition did leaves no pose to hold.
    for entity in released.read() {
        commands.entity(entity).try_remove::<MobUsePose>();
    }
    for end in ends.read() {
        let Ok((mut controller, model, overlay, entry)) = npcs.get_mut(end.who) else {
            continue;
        };
        if controller.current() != &end.what_animation {
            continue;
        }
        let Some(path) = Handle::path(&end.what_animation) else {
            continue;
        };
        let Some(ended) = animation_name_from_path(path.path()) else {
            continue;
        };
        if ended != entry.transition {
            continue;
        }
        debug!(
            "npc({}) mob pose({}) after transition({})",
            end.who, entry.pose, entry.transition
        );
        queue_named_animation(
            &mut controller,
            model,
            overlay,
            &files,
            &mut handles,
            &asset_server,
            &entry.pose,
        );
        commands.entity(end.who).remove::<MobUsePose>();
    }
}

/// Releases the current mob use when it is superseded or canceled.
pub(crate) fn start_use_mob_action(
    mut commands: Commands,
    mut npcs: Query<(Entity, &UseMobAction, Option<&NpcMobUse>), Added<UseMobAction>>,
    mut mobs: Query<&mut MobPose>,
) {
    for (entity, action, npc_mob_use) in &mut npcs {
        if action.state <= 0 {
            release_mob_occupancy(&mut commands, entity, npc_mob_use, &mut mobs);
            commands.entity(entity).remove::<ExecutingActionMarker>();
            continue;
        }
        let Some(mob_use) = npc_mob_use else {
            continue;
        };
        if mob_use.scheme == action.scheme {
            // The routine loop re-issues `AI_UseMob` until the body state
            // reports the use; drop the duplicate.
            commands.entity(entity).remove::<ExecutingActionMarker>();
            continue;
        }
        release_mob_occupancy(&mut commands, entity, Some(mob_use), &mut mobs);
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn run_use_mob_action(
    mut commands: Commands,
    gothic_script: Res<ScriptResource>,
    files: Res<AnimationFiles>,
    mut handles: ResMut<CachedAssetHandles>,
    asset_server: Res<AssetServer>,
    mut desired: MessageWriter<SetDesiredAnimation>,
    mob_registry: Res<MobRegistry>,
    mut mobs: Query<(
        Option<&GameMob>,
        &mut MobPose,
        &MobBase,
        &GlobalTransform,
        Option<&MobOccupant>,
        Option<&ZenGinModelComponent>,
    )>,
    mut npcs: Query<(
        Entity,
        &GameNpc,
        &UseMobAction,
        &mut Transform,
        Option<&NpcModel>,
        Option<&NpcOverlay>,
        &mut AutoAnimSequencer,
        &mut AnimationController,
        Option<&NpcMobUse>,
        Option<&NpcBodyRootOffset>,
        Has<PlayerCharacter>,
    )>,
) {
    let mut claimed_mobs = HashSet::new();
    for (
        entity,
        npc,
        action,
        mut transform,
        model,
        overlay,
        mut sequencer,
        mut controller,
        npc_mob_use,
        body_root_offset,
        is_player,
    ) in &mut npcs
    {
        let Some(model) = model else {
            continue;
        };
        if action.state <= 0 {
            if let Some(mob_use) = npc_mob_use
                && let Ok((_, mut pose, ..)) = mobs.get_mut(mob_use.mob)
            {
                pose.state = 0;
            }
            release_mob_use(
                &mut commands,
                entity,
                npc_mob_use,
                &mut controller,
                &mut sequencer,
            );
            commands.entity(entity).remove::<ExecutingActionMarker>();
            continue;
        }
        // A switch use can request state 0 while still attaching.
        let use_state = action.switch_state.unwrap_or(action.state).max(0);

        let Some(choice) = select_mob(
            &mut mobs,
            &mob_registry,
            &claimed_mobs,
            action.scheme.as_str(),
            transform.translation,
            action.target,
        ) else {
            debug!(
                "npc({}) AI_UseMob({}) found no available mob within {MOB_SEARCH_RADIUS}m",
                npc.instance_id(),
                action.scheme
            );
            commands.entity(entity).remove::<ExecutingActionMarker>();
            continue;
        };
        claimed_mobs.insert(choice.mob);

        let horizontal = horizontal_offset(transform.translation, choice.use_pos);
        if !action.snap && horizontal.length() > MOB_USE_DISTANCE {
            transform.rotation = face_direction_quat(horizontal);
            queue_root_motion_animation(
                &mut desired,
                entity,
                &mut controller,
                DesiredAnimation::Run,
                Some(horizontal.normalize_or_zero()),
            );
            continue;
        }

        if !choice.ready {
            // Attachment nodes are extracted once the model asset loads.
            continue;
        }
        // Doors are used from where the player stands. Regular pose mobs snap
        // to the attachment point; a `ZS_POS*` node marks the model root, while
        // the vob-transform fallback already sits on the ground.
        if !choice.no_move {
            let body_y_offset = if choice.attached {
                body_root_offset.map_or(NPC_BODY_Y_OFFSET, |offset| -offset.0 * transform.scale.y)
            } else {
                0.0
            };
            transform.translation = choice.use_pos + Vec3::Y * body_y_offset;
        }
        transform.rotation = choice.use_rot;
        sequencer.suspend();
        controller.set_root_motion(false);
        let pose = mob_animation_name(
            model,
            overlay,
            &files,
            action.scheme.as_str(),
            choice.position_tag.as_deref(),
            use_state,
        );
        // The engine enters the mob through the state transition and holds the
        // pose afterwards. Poses that do not animate the base bone (beds' FRONT
        // side, e.g. `S_BEDHIGH_FRONT_S1`) keep the root transform of the
        // transition, playing them alone leaves the npc standing.
        let mob_state = mobs
            .get(choice.mob)
            .map_or(0, |(_, pose, ..)| pose.state)
            .max(0);
        let transition = has_animation(model, overlay, &files, &pose)
            .then(|| {
                enter_animation_name(
                    model,
                    overlay,
                    &files,
                    action.scheme.as_str(),
                    choice.position_tag.as_deref(),
                    mob_state,
                    use_state,
                )
            })
            .flatten();
        if is_player {
            info!(
                "Player({}) uses mob({}) vob({}) scheme({}) state({}) animation({}) transition({transition:?})",
                npc.instance_id(),
                choice.mob,
                choice.vob_name,
                action.scheme,
                use_state,
                pose
            );
        }
        let first = transition.as_deref().unwrap_or(&pose);
        queue_named_animation(
            &mut controller,
            model,
            overlay,
            &files,
            &mut handles,
            &asset_server,
            first,
        );
        if let Some(transition) = transition {
            commands
                .entity(entity)
                .insert(MobUsePose { transition, pose });
        }
        commands.entity(choice.mob).insert(MobOccupant(entity));
        // Mob poses store their root translation relative to the unshifted mob
        // origin, while a skinned mob's visible mesh is drawn `rendered_origin`
        // away from the vob origin (the joint chain drops the hierarchy root
        // translation). Keep the actor entity at the `ZS_POS*` stand point
        // (camera, selection, release) and shift the skeleton so the pose lands
        // on the rendered mob instead.
        let rendered_origin = mobs.get(choice.mob).map_or(Vec3::ZERO, |mob| {
            let (_, pose, _, tr, ..) = mob;
            tr.rotation() * (pose.rendered_origin * tr.scale())
        });
        let mob_translation = mobs.get(choice.mob).map_or(transform.translation, |mob| {
            let (_, _, _, tr, ..) = mob;
            tr.translation()
        });
        let pose_offset = if choice.attached && !choice.no_move {
            choice.use_rot.inverse() * (mob_translation - rendered_origin - transform.translation)
        } else {
            Vec3::ZERO
        };
        commands.entity(entity).insert(NpcMobUse {
            mob: choice.mob,
            scheme: action.scheme.clone(),
            state: use_state,
            position_tag: choice.position_tag.clone(),
            pose_offset,
        });
        // The mob switches to the state the npc uses it in, which plays
        // its own `<STEM>-S_S<n>` animation.
        if let Ok((_, mut pose, ..)) = mobs.get_mut(choice.mob)
            && pose.state != use_state
        {
            pose.state = use_state;
        }
        gothic_script.lock().call_mob_state_hook(
            npc.instance_id(),
            &choice.on_state_func,
            use_state,
        );
        commands.entity(entity).remove::<ExecutingActionMarker>();
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn end_use_mob_action(
    mut removed: RemovedComponents<UseMobAction>,
    mut npcs: Query<(
        Option<&NpcMobUse>,
        &mut AutoAnimSequencer,
        &mut AnimationController,
    )>,
) {
    for entity in removed.read() {
        let Ok((npc_mob_use, mut sequencer, mut controller)) = npcs.get_mut(entity) else {
            continue;
        };
        // The action committed the mob use, the pose belongs to `NpcMobUse` now.
        if npc_mob_use.is_some() {
            continue;
        }
        controller.set_root_motion(false);
        queue_stand_animation(&mut sequencer);
    }
}

pub(crate) fn release_mob_use(
    commands: &mut Commands,
    npc_entity: Entity,
    mob_use: Option<&NpcMobUse>,
    controller: &mut AnimationController,
    sequencer: &mut AutoAnimSequencer,
) {
    let Some(mob_use) = mob_use else {
        return;
    };
    debug!("npc({npc_entity}) released mob({})", mob_use.mob);
    commands.entity(mob_use.mob).remove::<MobOccupant>();
    commands.entity(npc_entity).remove::<NpcMobUse>();
    controller.set_root_motion(false);
    queue_stand_animation(sequencer);
}

fn release_mob_occupancy(
    commands: &mut Commands,
    npc_entity: Entity,
    mob_use: Option<&NpcMobUse>,
    mobs: &mut Query<&mut MobPose>,
) {
    let Some(mob_use) = mob_use else {
        return;
    };
    debug!("npc({npc_entity}) released mob({})", mob_use.mob);
    commands.entity(mob_use.mob).remove::<MobOccupant>();
    commands.entity(npc_entity).remove::<NpcMobUse>();
    if let Ok(mut pose) = mobs.get_mut(mob_use.mob) {
        pose.state = 0;
    }
}

/// Available mob selected for an `AI_UseMob` request.
struct MobChoice {
    mob: Entity,
    vob_name: String,
    /// Script hook run when the npc reaches the mob's final state.
    on_state_func: String,
    use_pos: Vec3,
    use_rot: Quat,
    /// `FRONT`/`BACK` of the chosen `ZS_POS*` node, if it has one.
    position_tag: Option<String>,
    distance_squared: f32,
    /// Attachment nodes were extracted, the npc can attach.
    ready: bool,
    /// The position comes from a `ZS_POS*` node and marks the model root;
    /// the vob-transform fallback sits on the ground instead.
    attached: bool,
    /// Door: the user plays the pose from where they stand.
    no_move: bool,
}

/// Nearest free mob with the requested scheme within [`MOB_SEARCH_RADIUS`], or
/// exactly `target` when one is requested (the player's selected mob).
///
/// Mob requirements (`useWithItem`, `conditionFunc`) are not checked here: the
/// engine only enforces them for the player, npcs use a mob by scheme and a
/// free attachment point alone.
#[allow(clippy::type_complexity)]
fn select_mob(
    mobs: &mut Query<(
        Option<&GameMob>,
        &mut MobPose,
        &MobBase,
        &GlobalTransform,
        Option<&MobOccupant>,
        Option<&ZenGinModelComponent>,
    )>,
    registry: &MobRegistry,
    claimed: &HashSet<Entity>,
    scheme: &str,
    npc_pos: Vec3,
    target: Option<Entity>,
) -> Option<MobChoice> {
    if let Some(target) = target {
        return mob_choice(mobs, target, npc_pos);
    }
    // Script strings are canonical lowercased, the registry uses the uppercased
    // scheme derived from the visual name.
    let scheme = scheme.to_uppercase();
    let mut best: Option<MobChoice> = None;
    for &mob_entity in registry.mobs(&scheme) {
        if claimed.contains(&mob_entity) {
            continue;
        }
        let Ok((mob, pose, base, mob_tr, occupant, model)) = mobs.get_mut(mob_entity) else {
            continue;
        };
        if occupant.is_some() {
            continue;
        }
        let Some((use_pos, use_rot, position_tag, ready, attached)) =
            mob_use_transform(&pose, mob_tr, npc_pos, model.is_some())
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
                vob_name: base.vob_name.clone(),
                on_state_func: mob.map(|mob| mob.on_state_func.clone()).unwrap_or_default(),
                use_pos,
                use_rot,
                position_tag,
                distance_squared,
                ready,
                attached,
                no_move: pose.no_move,
            });
        }
    }
    best
}

/// Choice for one specific mob, used by player requests. A mob that is gone or
/// occupied yields `None`.
#[allow(clippy::type_complexity)]
fn mob_choice(
    mobs: &mut Query<(
        Option<&GameMob>,
        &mut MobPose,
        &MobBase,
        &GlobalTransform,
        Option<&MobOccupant>,
        Option<&ZenGinModelComponent>,
    )>,
    mob_entity: Entity,
    npc_pos: Vec3,
) -> Option<MobChoice> {
    let Ok((mob, pose, base, mob_tr, occupant, model)) = mobs.get_mut(mob_entity) else {
        return None;
    };
    if occupant.is_some() {
        return None;
    }
    let (use_pos, use_rot, position_tag, ready, attached) =
        mob_use_transform(&pose, mob_tr, npc_pos, model.is_some())?;
    Some(MobChoice {
        mob: mob_entity,
        vob_name: base.vob_name.clone(),
        on_state_func: mob.map(|mob| mob.on_state_func.clone()).unwrap_or_default(),
        use_pos,
        use_rot,
        position_tag,
        distance_squared: use_pos.distance_squared(npc_pos),
        ready,
        attached,
        no_move: pose.no_move,
    })
}

/// World transform the npc should use for the mob, using the nearest `ZS_POS*`
/// node. Mobs without attachment nodes fall back to the vob transform.
///
/// The last tuple element is `true` when the position is a `ZS_POS*` node: it
/// marks the model root, so the actor has to subtract its body-root offset.
/// The vob-transform fallback sits on the ground and needs no offset.
fn mob_use_transform(
    pose: &MobPose,
    mob_tr: &GlobalTransform,
    npc_pos: Vec3,
    has_model: bool,
) -> Option<(Vec3, Quat, Option<String>, bool, bool)> {
    let Some(positions) = pose.positions.as_ref() else {
        // A model without extracted nodes is still loading; the npc waits at
        // the vob transform and attaches once the asset is scanned.
        return Some((
            mob_tr.translation(),
            mob_tr.rotation(),
            None,
            !has_model,
            false,
        ));
    };
    if positions.is_empty() {
        return Some((mob_tr.translation(), mob_tr.rotation(), None, true, false));
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
    Some((world.translation(), world.rotation(), tag, true, true))
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

/// Mob-use pose animation name, e.g. `S_BENCH_S1` or `S_BEDHIGH_FRONT_S1`.
///
/// Only beds ship position-tagged poses; other mobs (e.g. `BAUMSAEGE`) have the
/// attachment node but only the untagged animation, so the tag is used only
/// when the archive has it.
pub(crate) fn mob_animation_name(
    model: &NpcModel,
    overlay: Option<&NpcOverlay>,
    files: &AnimationFiles,
    scheme: &str,
    position_tag: Option<&str>,
    state: i32,
) -> String {
    let state = state.max(0);
    let scheme = scheme.to_uppercase();
    let tagged = position_tag.map(|tag| format!("S_{scheme}_{tag}_S{state}"));
    if let Some(tagged) = tagged
        && has_animation(model, overlay, files, &tagged)
    {
        return tagged;
    }
    format!("S_{scheme}_S{state}")
}

/// Entering transition `T_<SCHEME>[_<POS>]_S<from>_2_S<to>`, if the archive has
/// it, tagged first like [`mob_animation_name`].
pub(crate) fn enter_animation_name(
    model: &NpcModel,
    overlay: Option<&NpcOverlay>,
    files: &AnimationFiles,
    scheme: &str,
    position_tag: Option<&str>,
    from_state: i32,
    to_state: i32,
) -> Option<String> {
    if from_state == to_state {
        return None;
    }
    let scheme = scheme.to_uppercase();
    let tagged = position_tag.map(|tag| format!("T_{scheme}_{tag}_S{from_state}_2_S{to_state}"));
    if let Some(tagged) = tagged
        && has_animation(model, overlay, files, &tagged)
    {
        return Some(tagged);
    }
    let name = format!("T_{scheme}_S{from_state}_2_S{to_state}");
    has_animation(model, overlay, files, &name).then_some(name)
}

/// Whether the archive has a player pose for the mob scheme, tagged or not.
///
/// Used by the player interaction to decide between attaching to the mob and
/// the plain state toggle of doors/levers/wheels. The exact tag is resolved
/// when the attachment nodes are available (`mob_animation_name`).
pub(crate) fn mob_scheme_has_player_pose(
    model: &NpcModel,
    overlay: Option<&NpcOverlay>,
    files: &AnimationFiles,
    scheme: &str,
    state: i32,
) -> bool {
    let state = state.max(0);
    let scheme = scheme.to_uppercase();
    [None, Some("FRONT"), Some("BACK")].into_iter().any(|tag| {
        let name = match tag {
            Some(tag) => format!("S_{scheme}_{tag}_S{state}"),
            None => format!("S_{scheme}_S{state}"),
        };
        has_animation(model, overlay, files, &name)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Resource, Default)]
    struct Selected(Option<Entity>);

    #[derive(Resource, Default)]
    struct Requested(Option<Entity>);

    fn select(mut selected: ResMut<Selected>, registry: Res<MobRegistry>, mut mobs: MobQuery) {
        let claimed = HashSet::new();
        selected.0 = select_mob(&mut mobs, &registry, &claimed, "bsanvil", Vec3::ZERO, None)
            .map(|choice| choice.mob);
    }

    fn select_requested(
        mut selected: ResMut<Selected>,
        registry: Res<MobRegistry>,
        requested: Res<Requested>,
        mut mobs: MobQuery,
    ) {
        let claimed = HashSet::new();
        selected.0 = select_mob(
            &mut mobs,
            &registry,
            &claimed,
            "bsanvil",
            Vec3::ZERO,
            requested.0,
        )
        .map(|choice| choice.mob);
    }

    type MobQuery<'w, 's> = Query<
        'w,
        's,
        (
            Option<&'static GameMob>,
            &'static mut MobPose,
            &'static MobBase,
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
            game_pose("BSANVIL"),
            MobBase::default(),
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

    fn game_mob(_scheme: &str, use_with_item: &str) -> GameMob {
        GameMob {
            use_with_item: use_with_item.to_string(),
            on_state_func: String::new(),
            condition_func: String::new(),
            rewind: false,
            trigger_target: String::new(),
            trigger_fired: false,
            auto_end: false,
        }
    }

    fn game_pose(scheme: &str) -> MobPose {
        MobPose {
            scheme: scheme.to_string(),
            visual_stem: scheme.to_uppercase(),
            state_num: 1,
            ..default()
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

    #[test]
    fn requested_mob_wins_over_a_nearer_one() {
        let mut app = App::new();
        app.init_resource::<MobRegistry>();
        app.init_resource::<Selected>();
        app.init_resource::<Requested>();
        app.add_systems(Update, select_requested);

        let near = app
            .world_mut()
            .spawn((
                game_mob("BSANVIL", ""),
                game_pose("BSANVIL"),
                MobBase::default(),
                Transform::IDENTITY,
                GlobalTransform::IDENTITY,
            ))
            .id();
        let far = app
            .world_mut()
            .spawn((
                game_mob("BSANVIL", ""),
                game_pose("BSANVIL"),
                MobBase::default(),
                Transform::from_xyz(5.0, 0.0, 0.0),
                GlobalTransform::from_xyz(5.0, 0.0, 0.0),
            ))
            .id();
        app.world_mut()
            .resource_mut::<MobRegistry>()
            .insert("BSANVIL".to_string(), far);
        app.world_mut()
            .resource_mut::<MobRegistry>()
            .insert("BSANVIL".to_string(), near);
        app.world_mut().resource_mut::<Requested>().0 = Some(far);
        app.update();

        assert_eq!(app.world().resource::<Selected>().0, Some(far));
    }

    #[test]
    fn requested_occupied_mob_is_rejected() {
        let mut app = App::new();
        app.init_resource::<MobRegistry>();
        app.init_resource::<Selected>();
        app.init_resource::<Requested>();
        app.add_systems(Update, select_requested);

        let mob = app
            .world_mut()
            .spawn((
                game_mob("BSANVIL", ""),
                game_pose("BSANVIL"),
                MobBase::default(),
                Transform::IDENTITY,
                GlobalTransform::IDENTITY,
                MobOccupant(Entity::PLACEHOLDER),
            ))
            .id();
        app.world_mut()
            .resource_mut::<MobRegistry>()
            .insert("BSANVIL".to_string(), mob);
        app.world_mut().resource_mut::<Requested>().0 = Some(mob);
        app.update();

        assert_eq!(app.world().resource::<Selected>().0, None);
    }

    #[test]
    fn player_pose_detection_covers_tagged_untagged_and_doors() {
        let model = NpcModel {
            animation_scheme: Some("HUMANS".to_string()),
            ..Default::default()
        };
        let files = AnimationFiles(HashSet::from([
            "HUMANS-S_CHAIR_S1.MAN".to_string(),
            "HUMANS-S_BEDHIGH_BACK_S1.MAN".to_string(),
        ]));

        assert!(mob_scheme_has_player_pose(&model, None, &files, "CHAIR", 1));
        assert!(mob_scheme_has_player_pose(
            &model, None, &files, "BEDHIGH", 1
        ));
        assert!(!mob_scheme_has_player_pose(&model, None, &files, "DOOR", 1));
    }

    #[test]
    fn player_pose_detection_accepts_the_switch_state_zero() {
        let model = NpcModel {
            animation_scheme: Some("HUMANS".to_string()),
            ..Default::default()
        };
        let files = AnimationFiles(HashSet::from([
            "HUMANS-S_LEVER_S0.MAN".to_string(),
            "HUMANS-S_LEVER_S1.MAN".to_string(),
        ]));

        assert!(mob_scheme_has_player_pose(&model, None, &files, "LEVER", 0));
        assert!(mob_scheme_has_player_pose(&model, None, &files, "LEVER", 1));
        assert_eq!(
            mob_animation_name(&model, None, &files, "LEVER", None, 0),
            "S_LEVER_S0"
        );
    }

    #[test]
    fn player_switch_stays_an_attach_request_at_state_zero() {
        let action = UseMobAction::player_switch(Entity::PLACEHOLDER, StringCanon::new("LEVER"), 0);

        assert!(action.state > 0, "state 0 must not mean detach");
        assert_eq!(action.switch_state, Some(0));
        assert!(action.snap);
    }

    #[test]
    fn enter_transition_prefers_the_position_tag_and_falls_back() {
        let model = NpcModel {
            animation_scheme: Some("HUMANS".to_string()),
            ..Default::default()
        };

        let beds = AnimationFiles(HashSet::from([
            "HUMANS-T_BEDHIGH_FRONT_S0_2_S1.MAN".to_string(),
            "HUMANS-T_BEDHIGH_S0_2_S1.MAN".to_string(),
        ]));
        assert_eq!(
            enter_animation_name(&model, None, &beds, "BEDHIGH", Some("FRONT"), 0, 1).as_deref(),
            Some("T_BEDHIGH_FRONT_S0_2_S1")
        );
        // The `_BACK` attachment node has no transition, the untagged one is used.
        assert_eq!(
            enter_animation_name(&model, None, &beds, "BEDHIGH", Some("BACK"), 0, 1).as_deref(),
            Some("T_BEDHIGH_S0_2_S1")
        );
    }

    #[test]
    fn enter_transition_requires_a_state_change_and_an_existing_file() {
        let model = NpcModel {
            animation_scheme: Some("HUMANS".to_string()),
            ..Default::default()
        };

        // A lever has no `T_LEVER_S1_2_S0`, the pose is played directly.
        let levers = AnimationFiles(HashSet::from(["HUMANS-T_LEVER_S0_2_S1.MAN".to_string()]));
        assert_eq!(
            enter_animation_name(&model, None, &levers, "LEVER", None, 1, 0),
            None
        );
        assert_eq!(
            enter_animation_name(&model, None, &levers, "LEVER", None, 1, 1),
            None
        );
    }

    #[test]
    fn mob_animation_uses_the_position_tag_only_when_it_exists() {
        let model = NpcModel {
            animation_scheme: Some("HUMANS".to_string()),
            ..Default::default()
        };

        let beds = AnimationFiles(HashSet::from(["HUMANS-S_BEDHIGH_BACK_S1.MAN".to_string()]));
        assert_eq!(
            mob_animation_name(&model, None, &beds, "BEDHIGH", Some("BACK"), 1),
            "S_BEDHIGH_BACK_S1"
        );

        // The sawmill has a `_BACK` attachment node but only an untagged pose.
        let sawmill = AnimationFiles(HashSet::from(["HUMANS-S_BAUMSAEGE_S1.MAN".to_string()]));
        assert_eq!(
            mob_animation_name(&model, None, &sawmill, "BAUMSAEGE", Some("BACK"), 1),
            "S_BAUMSAEGE_S1"
        );
    }
}
