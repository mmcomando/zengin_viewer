//! Player use of mobs: stand-up transitions and the physics freeze while
//! attached.
//!
//! The attach itself is the shared npc [`UseMobAction`](super::super::npc_actions::UseMobAction),
//! so the pose, occupancy and body state come from the npc path. Leaving the
//! mob plays the release transition `T_<SCHEME>[_<POS>]_S<n>_2_STAND` (stepping
//! down a state when the scheme has no direct one) and only then restores the
//! standing idle, mirroring the `AI_UseItemToState` release.

use avian3d::prelude::*;
use bevy::prelude::*;
use leafwing_input_manager::prelude::ActionState;

use crate::gameplay::objects::{NpcModel, PlayerCharacter};
use crate::graphics::{
    AnimationController, AnimationEnds, AutoAnimSequencer, animation_name_from_path,
};
use crate::gui::{InventoryUiState, LootClosed};
use crate::input::{GameplayAction, GameplayInputMarker};
use crate::zengin::npc_actions::{
    ExecutingActionMarker, MobUsePose, UseMobAction, mob_animation_name, release_mob_use,
};
use crate::zengin::npc_routines::{
    AnimationFiles, NpcOverlay, has_animation, queue_named_animation, queue_stand_animation,
};
use crate::zengin::resources::CachedAssetHandles;
use crate::zengin::script::component::ScriptResource;
use crate::zengin::{GameMob, GameNpc, MobPose, NpcMobUse};

/// Player is playing the stand-up transition; the mob use is released when it
/// ends.
#[derive(Component, Debug)]
pub(crate) struct PlayerMobRelease {
    /// Animation whose end advances the release, `None` finishes immediately.
    expected: Option<String>,
    /// Pose state the player is leaving, stepped down when needed.
    state: i32,
    elapsed: f32,
    /// Minimum time to hold the animation before advancing. Switch poses are
    /// often a single frame; without the hold their use would not be visible.
    hold: f32,
}

/// Stand-up transitions never legitimately take this long; the timeout keeps a
/// player from being stuck attached when the animation system stops updating
/// (e.g. the visual went off screen).
const RELEASE_TIMEOUT_SECS: f32 = 3.0;

/// Switch use poses are often single-frame; keep them on screen briefly before
/// the automatic stand-up.
const AUTO_END_HOLD_SECS: f32 = 0.2;

/// Queues the stand-up transition, or frees the mob when the scheme has none.
/// Returns `true` when the release is complete.
#[allow(clippy::too_many_arguments)]
fn advance_release(
    release: &mut PlayerMobRelease,
    mob_use: &NpcMobUse,
    controller: &mut AnimationController,
    model: &NpcModel,
    overlay: Option<&NpcOverlay>,
    files: &AnimationFiles,
    handles: &mut CachedAssetHandles,
    asset_server: &AssetServer,
) -> bool {
    // A `S<n>_2_STAND` transition ends in the standing pose, the release is done.
    let stand = release_animation_name(mob_use, release.state);
    if release.expected.as_deref() == Some(stand.as_str()) {
        return true;
    }
    let Some((animation, state)) = next_release_step(mob_use, release.state, |name| {
        has_animation(model, overlay, files, name)
    }) else {
        return true;
    };
    queue_named_animation(
        controller,
        model,
        overlay,
        files,
        handles,
        asset_server,
        &animation,
    );
    release.state = state;
    release.expected = Some(animation);
    release.hold = 0.0;
    false
}

/// Next animation towards the standing pose: the `S<n>_2_STAND` transition,
/// otherwise a one-state step down. `None` when the scheme has neither.
fn next_release_step(
    mob_use: &NpcMobUse,
    state: i32,
    has: impl Fn(&str) -> bool,
) -> Option<(String, i32)> {
    let stand = release_animation_name(mob_use, state);
    if has(&stand) {
        return Some((stand, state));
    }
    if state > 0 {
        let next = state - 1;
        let step = step_animation_name(mob_use, state, next);
        if has(&step) {
            return Some((step, next));
        }
    }
    None
}

fn release_animation_name(mob_use: &NpcMobUse, state: i32) -> String {
    let scheme = mob_use.scheme.as_str().to_uppercase();
    match mob_use.position_tag.as_deref() {
        Some(tag) => format!("T_{scheme}_{tag}_S{state}_2_STAND"),
        None => format!("T_{scheme}_S{state}_2_STAND"),
    }
}

fn step_animation_name(mob_use: &NpcMobUse, from: i32, to: i32) -> String {
    let scheme = mob_use.scheme.as_str().to_uppercase();
    match mob_use.position_tag.as_deref() {
        Some(tag) => format!("T_{scheme}_{tag}_S{from}_2_S{to}"),
        None => format!("T_{scheme}_S{from}_2_S{to}"),
    }
}

/// Leaves the chest when the loot panels close: closes the lid immediately,
/// cancels a not-yet-attached use, or starts the stand-up transition.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn close_looted_chest(
    mut commands: Commands,
    mut closed: MessageReader<LootClosed>,
    files: Option<Res<AnimationFiles>>,
    mut handles: ResMut<CachedAssetHandles>,
    asset_server: Res<AssetServer>,
    gothic_script: Res<ScriptResource>,
    mut mobs: Query<(Option<&GameMob>, &mut MobPose)>,
    mut players: Query<
        (
            Entity,
            &GameNpc,
            Option<&NpcMobUse>,
            Option<&PlayerMobRelease>,
            Option<&mut AnimationController>,
            Option<&mut AutoAnimSequencer>,
            Option<&NpcModel>,
            Option<&NpcOverlay>,
            Has<UseMobAction>,
        ),
        With<PlayerCharacter>,
    >,
) {
    for request in closed.read() {
        // The chest may be gone when the world was unloaded mid-loot.
        if mobs.get(request.chest).is_err() {
            continue;
        }
        // The lid closes in parallel with the player stand-up.
        if let Ok((_, mut pose)) = mobs.get_mut(request.chest) {
            pose.state = 0;
        }

        let Ok((entity, npc, mob_use, release, controller, sequencer, model, overlay, pending_use)) =
            players.single_mut()
        else {
            continue;
        };
        if release.is_some() {
            continue;
        }
        let Some(mob_use) = mob_use else {
            // The loot UI can close before the attach commits.
            if pending_use {
                commands.entity(entity).remove::<ExecutingActionMarker>();
            }
            continue;
        };
        if mob_use.mob != request.chest {
            continue;
        }
        let (Some(mut controller), Some(mut sequencer)) = (controller, sequencer) else {
            continue;
        };
        let mut release = PlayerMobRelease {
            expected: None,
            state: mob_use.state.max(0),
            elapsed: 0.0,
            hold: 0.0,
        };
        let finished = match (model, files.as_deref()) {
            (Some(model), Some(files)) => advance_release(
                &mut release,
                mob_use,
                &mut controller,
                model,
                overlay,
                files,
                &mut handles,
                &asset_server,
            ),
            _ => true,
        };
        if finished {
            finish_player_mob_release(
                &mut commands,
                entity,
                npc,
                mob_use,
                &mut controller,
                &mut sequencer,
                &mut mobs,
                &gothic_script,
            );
            continue;
        }
        info!(
            "Player({}) leaves chest({}) transition({:?})",
            npc.instance_id(),
            mob_use.mob,
            release.expected
        );
        commands.entity(entity).remove::<MobUsePose>();
        commands.entity(entity).insert(release);
    }
}

/// Releases a player left attached to an entity that no longer exists, e.g.
/// after a world unload.
#[allow(clippy::type_complexity)]
pub(super) fn release_stale_player_mob_use(
    mut commands: Commands,
    mut players: Query<
        (
            Entity,
            &NpcMobUse,
            Option<&mut AnimationController>,
            Option<&mut AutoAnimSequencer>,
        ),
        With<PlayerCharacter>,
    >,
    mobs: Query<(), With<MobPose>>,
) {
    for (entity, mob_use, controller, sequencer) in &mut players {
        if mobs.get(mob_use.mob).is_ok() {
            continue;
        }
        warn!("Player mob use target({}) is gone, releasing", mob_use.mob);
        commands.entity(entity).remove::<(
            NpcMobUse,
            PlayerMobRelease,
            MobUsePose,
            ExecutingActionMarker,
        )>();
        if let Some(mut controller) = controller {
            controller.set_root_motion(true);
        }
        if let Some(mut sequencer) = sequencer {
            queue_stand_animation(&mut sequencer);
        }
    }
}

/// Starts the player's stand-up when it asks to leave the mob. Movement and
/// attack input also cancel a use that has not attached yet.
#[allow(clippy::type_complexity)]
pub(super) fn stand_up_player_from_mob(
    mut commands: Commands,
    action_state: Single<&ActionState<GameplayAction>, With<GameplayInputMarker>>,
    files: Option<Res<AnimationFiles>>,
    mut handles: ResMut<CachedAssetHandles>,
    asset_server: Res<AssetServer>,
    inventory_ui: Res<InventoryUiState>,
    pending: Query<Entity, (With<PlayerCharacter>, With<UseMobAction>)>,
    mut mobs: Query<(Option<&GameMob>, &mut MobPose)>,
    gothic_script: Res<ScriptResource>,
    mut players: Query<
        (
            Entity,
            &GameNpc,
            &NpcMobUse,
            Option<&PlayerMobRelease>,
            Option<&mut AnimationController>,
            Option<&mut AutoAnimSequencer>,
            Option<&NpcModel>,
            Option<&NpcOverlay>,
        ),
        With<PlayerCharacter>,
    >,
) {
    // The loot UI owns the input while it is open; its close path releases the
    // chest instead.
    if inventory_ui.is_open() {
        return;
    }
    let moving = action_state
        .axis_pair(&GameplayAction::Move)
        .length_squared()
        > 0.1;
    let attacking = action_state.just_pressed(&GameplayAction::AttackLeft)
        || action_state.just_pressed(&GameplayAction::AttackRight)
        || action_state.just_pressed(&GameplayAction::DealDamage);
    let interacting = action_state.just_pressed(&GameplayAction::Interact);

    if moving || attacking {
        for entity in &pending {
            commands.entity(entity).remove::<ExecutingActionMarker>();
        }
    }

    for (entity, npc, mob_use, release, controller, sequencer, model, overlay) in &mut players {
        if release.is_some() {
            continue;
        }
        if !(interacting || moving || attacking) {
            continue;
        }
        let (Some(mut controller), Some(mut sequencer)) = (controller, sequencer) else {
            continue;
        };
        let mut release = PlayerMobRelease {
            expected: None,
            state: mob_use.state.max(0),
            elapsed: 0.0,
            hold: 0.0,
        };
        let finished = match (model, files.as_deref()) {
            (Some(model), Some(files)) => advance_release(
                &mut release,
                mob_use,
                &mut controller,
                model,
                overlay,
                files,
                &mut handles,
                &asset_server,
            ),
            _ => true,
        };
        if finished {
            finish_player_mob_release(
                &mut commands,
                entity,
                npc,
                mob_use,
                &mut controller,
                &mut sequencer,
                &mut mobs,
                &gothic_script,
            );
            continue;
        }
        info!(
            "Player({}) stands up from mob({}) transition({:?})",
            npc.instance_id(),
            mob_use.mob,
            release.expected
        );
        // The stand-up replaces the entering transition, its pose must not
        // follow afterwards.
        commands.entity(entity).remove::<MobUsePose>();
        commands.entity(entity).insert(release);
    }
}

/// Starts the automatic stand-up of switch-like mobs (levers, buttons, wheels,
/// doors) once their use pose is queued.
#[allow(clippy::type_complexity)]
pub(super) fn start_auto_end_release(
    mut commands: Commands,
    files: Res<AnimationFiles>,
    mobs: Query<&GameMob>,
    players: Query<
        (Entity, &GameNpc, &NpcMobUse, &NpcModel, Option<&NpcOverlay>),
        (
            With<PlayerCharacter>,
            Added<NpcMobUse>,
            Without<PlayerMobRelease>,
        ),
    >,
) {
    for (entity, npc, mob_use, model, overlay) in &players {
        let Ok(mob) = mobs.get(mob_use.mob) else {
            continue;
        };
        if !mob.auto_end {
            continue;
        }
        let pose = mob_animation_name(
            model,
            overlay,
            &files,
            mob_use.scheme.as_str(),
            mob_use.position_tag.as_deref(),
            mob_use.state,
        );
        info!(
            "Player({}) auto-ends mob({}) use, pose({pose})",
            npc.instance_id(),
            mob_use.mob
        );
        commands.entity(entity).insert(PlayerMobRelease {
            expected: Some(pose),
            state: mob_use.state,
            elapsed: 0.0,
            hold: AUTO_END_HOLD_SECS,
        });
    }
}

/// Advances the stand-up transition on its animation end, or releases the mob
/// on timeout.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn advance_player_mob_release(
    time: Res<Time>,
    mut commands: Commands,
    mut ends: MessageReader<AnimationEnds>,
    files: Res<AnimationFiles>,
    mut handles: ResMut<CachedAssetHandles>,
    asset_server: Res<AssetServer>,
    gothic_script: Res<ScriptResource>,
    mut mobs: Query<(Option<&GameMob>, &mut MobPose)>,
    mut players: Query<(
        Entity,
        &GameNpc,
        &NpcMobUse,
        &mut PlayerMobRelease,
        &mut AnimationController,
        &NpcModel,
        Option<&NpcOverlay>,
        &mut AutoAnimSequencer,
    )>,
) {
    let mut ended: Vec<(Entity, String)> = Vec::new();
    for end in ends.read() {
        let Some(path) = Handle::path(&end.what_animation) else {
            continue;
        };
        let Some(name) = animation_name_from_path(path.path()) else {
            continue;
        };
        ended.push((end.who, name.to_string()));
    }

    let delta = time.delta_secs();
    for (entity, npc, mob_use, mut release, mut controller, model, overlay, mut sequencer) in
        &mut players
    {
        release.elapsed += delta;
        let timed_out = release.elapsed > RELEASE_TIMEOUT_SECS;
        let played = release.elapsed >= release.hold
            && release.expected.as_ref().is_some_and(|expected| {
                ended
                    .iter()
                    .any(|(who, name)| *who == entity && name == expected)
            });
        let finished = if timed_out {
            warn!(
                "Player({}) stand-up transition({:?}) timed out",
                npc.instance_id(),
                release.expected
            );
            true
        } else if played {
            advance_release(
                &mut release,
                mob_use,
                &mut controller,
                model,
                overlay,
                &files,
                &mut handles,
                &asset_server,
            )
        } else {
            false
        };
        if finished {
            finish_player_mob_release(
                &mut commands,
                entity,
                npc,
                mob_use,
                &mut controller,
                &mut sequencer,
                &mut mobs,
                &gothic_script,
            );
        }
    }
}

/// Frees the mob, restores the standing idle and runs the `onStateFunc` hook
/// with the released state.
#[allow(clippy::too_many_arguments)]
fn finish_player_mob_release(
    commands: &mut Commands,
    entity: Entity,
    npc: &GameNpc,
    mob_use: &NpcMobUse,
    controller: &mut AnimationController,
    sequencer: &mut AutoAnimSequencer,
    mobs: &mut Query<(Option<&GameMob>, &mut MobPose)>,
    gothic_script: &ScriptResource,
) {
    // Switch-like mobs keep the state their use applied; only regular pose mobs
    // return to state 0 and report the release to `onStateFunc`.
    let auto_end = mobs
        .get(mob_use.mob)
        .is_ok_and(|(mob, _)| mob.is_some_and(|mob| mob.auto_end));
    let on_state_func = if auto_end {
        None
    } else {
        match mobs.get_mut(mob_use.mob) {
            Ok((mob, mut pose)) => {
                pose.state = 0;
                mob.map(|mob| mob.on_state_func.clone())
            }
            Err(_) => None,
        }
    };
    info!(
        "Player({}) released mob({})",
        npc.instance_id(),
        mob_use.mob
    );
    release_mob_use(commands, entity, Some(mob_use), controller, sequencer);
    controller.set_root_motion(true);
    commands.entity(entity).remove::<PlayerMobRelease>();
    if let Some(on_state_func) = on_state_func {
        gothic_script
            .lock()
            .call_mob_state_hook(npc.instance_id(), &on_state_func, 0);
    }
}

/// Freezes the player while attached: the kinematic capsule is disabled so the
/// pose is not pushed out of the mob, and gravity/move-and-slide skip the
/// player through the [`NpcMobUse`] filter.
pub(super) fn sync_player_mob_physics(
    mut commands: Commands,
    added: Query<Entity, (With<PlayerCharacter>, Added<NpcMobUse>)>,
    mut removed: RemovedComponents<NpcMobUse>,
    mut velocities: Query<&mut LinearVelocity, With<PlayerCharacter>>,
) {
    for entity in &added {
        commands.entity(entity).try_insert(ColliderDisabled);
        if let Ok(mut velocity) = velocities.get_mut(entity) {
            velocity.0 = Vec3::ZERO;
        }
    }
    for entity in removed.read() {
        commands.entity(entity).try_remove::<ColliderDisabled>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zengin::script::string_value::StringCanon;

    fn mob_use(scheme: &str, tag: Option<&str>, state: i32) -> NpcMobUse {
        NpcMobUse {
            mob: Entity::PLACEHOLDER,
            scheme: StringCanon::new(scheme),
            state,
            position_tag: tag.map(str::to_string),
            pose_offset: Vec3::ZERO,
            cancel_root_translation: false,
        }
    }

    #[test]
    fn release_prefers_the_stand_transition() {
        let mob = mob_use("BSANVIL", None, 1);
        let (animation, state) =
            next_release_step(&mob, 1, |name| name == "T_BSANVIL_S1_2_STAND").unwrap();

        assert_eq!(animation, "T_BSANVIL_S1_2_STAND");
        assert_eq!(state, 1);
    }

    #[test]
    fn release_steps_down_without_a_stand_transition() {
        let mob = mob_use("BSANVIL", None, 1);
        let (animation, state) =
            next_release_step(&mob, 1, |name| name == "T_BSANVIL_S1_2_S0").unwrap();

        assert_eq!(animation, "T_BSANVIL_S1_2_S0");
        assert_eq!(state, 0);

        // From the reached state 0 the release transition is the next step.
        let (animation, state) =
            next_release_step(&mob, 0, |name| name == "T_BSANVIL_S0_2_STAND").unwrap();
        assert_eq!(animation, "T_BSANVIL_S0_2_STAND");
        assert_eq!(state, 0);
    }

    #[test]
    fn release_without_transitions_finishes() {
        let mob = mob_use("CHAIR", None, 1);

        assert!(next_release_step(&mob, 1, |_| false).is_none());
    }

    #[test]
    fn tagged_release_keeps_the_position_tag() {
        let mob = mob_use("BEDHIGH", Some("BACK"), 1);

        assert_eq!(release_animation_name(&mob, 1), "T_BEDHIGH_BACK_S1_2_STAND");
        assert_eq!(step_animation_name(&mob, 1, 0), "T_BEDHIGH_BACK_S1_2_S0");
    }

    #[test]
    fn chest_release_steps_down_to_s0_without_a_stand_transition() {
        // Chests ship `T_CHESTBIG_S1_2_S0` but no `S1_2_STAND`.
        let mob = mob_use("CHESTBIG", None, 1);
        let has = |name: &str| name == "T_CHESTBIG_S1_2_S0";

        let (animation, state) = next_release_step(&mob, 1, has).unwrap();
        assert_eq!((animation.as_str(), state), ("T_CHESTBIG_S1_2_S0", 0));
        assert!(next_release_step(&mob, 0, has).is_none());
    }

    #[test]
    fn stale_player_mob_use_is_released() {
        let mut app = App::new();
        app.add_systems(Update, release_stale_player_mob_use);
        let player = app
            .world_mut()
            .spawn((PlayerCharacter, mob_use("CHESTBIG", None, 1)))
            .id();

        app.update();

        assert!(app.world().get::<NpcMobUse>(player).is_none());
    }
}
