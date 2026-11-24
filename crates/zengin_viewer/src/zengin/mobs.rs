//! Interactive mob registry and mesh attachment-point extraction.

use std::collections::HashMap;

use avian3d::prelude::{Collider, CollisionLayers};
use bevy::camera::primitives::Aabb;
use bevy::prelude::*;

use crate::gameplay::distance::DistanceNear;
use crate::gameplay::game_time::GameTime;
use crate::graphics::{
    AnimationController, AnimationData, AnimationEnds, animation_name_from_path,
};
use crate::perf_debug;
use crate::zengin::GameChest;
use crate::zengin::GameMob;
use crate::zengin::GameNpc;
use crate::zengin::MobBase;
use crate::zengin::MobPose;
use crate::zengin::common::{animation_file_name, get_animation_path};
use crate::zengin::interaction::selection::HitColliderOwner;
use crate::zengin::npc_actions::NpcItemUse;
use crate::zengin::npc_routines::AnimationFiles;
use crate::zengin::resources::CachedAssetHandles;
use crate::zengin::resources::{SELECTABLE, ZenGinModelComponent, ZenGinModelComponentLoaded};
use crate::zengin::script::component::ScriptResource;
use crate::zengin::script::state::ScriptMob;
use crate::zengin::script::state::{BS_ITEMINTERACT, MobRoutineEntry};
use crate::zengin::script::string_value::StringCanon;
use crate::zengin::visual::model::ZenGinModel;

/// Lower bound of a mob selection collider half extent, in meters, so tiny
/// vobs stay selectable.
const MIN_MOB_COLLIDER_HALF_EXTENT: f32 = 0.2;

type MobVisualQuery = (Entity, &'static Aabb);

pub struct MobsPlugin;

impl Plugin for MobsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MobRegistry>();
        app.add_systems(
            Update,
            (
                index_added_mobs,
                unindex_removed_mobs,
                extract_mob_positions,
                release_despawned_mob_uses,
                sync_npc_body_state,
                apply_mob_routines,
                apply_mob_animations,
                advance_mob_state_transition,
                sync_script_mobs,
                add_mob_selection_colliders,
            ),
        );
    }
}

/// Runtime index of interactive mobs by their `AI_UseMob` scheme.
#[derive(Resource, Default, Debug)]
pub struct MobRegistry {
    by_scheme: HashMap<String, Vec<Entity>>,
}

impl MobRegistry {
    pub fn insert(&mut self, scheme: String, entity: Entity) {
        self.by_scheme.entry(scheme).or_default().push(entity);
    }

    pub fn remove_entity(&mut self, entity: Entity) {
        self.by_scheme.retain(|_, entities| {
            entities.retain(|mob| *mob != entity);
            !entities.is_empty()
        });
    }

    pub fn mobs(&self, scheme: &str) -> &[Entity] {
        self.by_scheme.get(scheme).map_or(&[], Vec::as_slice)
    }

    #[allow(dead_code)] // consumed by the mob use action
    pub fn iter(&self) -> impl Iterator<Item = (&str, &[Entity])> {
        self.by_scheme
            .iter()
            .map(|(scheme, mobs)| (scheme.as_str(), mobs.as_slice()))
    }
}

/// Npc currently attached to a mob interaction point.
#[derive(Component, Debug, Clone)]
pub struct NpcMobUse {
    pub mob: Entity,
    pub scheme: StringCanon,
    /// Mob state the npc uses the mob in.
    pub state: i32,
    /// `FRONT`/`BACK` of the attachment point, if it has one.
    pub position_tag: Option<String>,
    /// Skeleton-local offset aligning the mob-relative pose animation with the
    /// entity standing at the `ZS_POS*` attachment point.
    pub pose_offset: Vec3,
    /// Whether the skeleton must cancel the pose animation's horizontal root
    /// translation. Set for standing work poses, which keep the actor on its
    /// `ZS_POS*` stand point; `false` for seated/lying poses, which keep the
    /// pose root and anchor the body at the mob origin.
    pub cancel_root_translation: bool,
}

/// Npc entity occupying a mob interaction point.
#[derive(Component, Debug, Clone, Copy)]
pub struct MobOccupant(pub Entity);

/// Model of a mob whose vob has `showVisual = false`. It is loaded only to
/// read its `ZS_POS*` attachment nodes and never turned into mesh entities.
#[derive(Component, Debug, Clone)]
pub struct MobHiddenModel(pub Handle<ZenGinModel>);

/// Animation currently applied to a mob for its state.
#[derive(Component, Debug, Clone)]
pub struct MobAnimation {
    /// Visual file stem, the `<STEM>` of the `<STEM>-S_S<n>.MAN` set.
    pub stem: String,
    /// State the animation handle of the model belongs to.
    pub state: i32,
}

/// Transition playing before the loop of `to` is queued.
#[derive(Component, Debug, Clone)]
struct MobStateTransition {
    /// Short animation name, e.g. `T_S1_2_S0`.
    expected: String,
    to: i32,
}

/// Registers freshly spawned mobs under their `AI_UseMob` scheme.
fn index_added_mobs(
    mut registry: ResMut<MobRegistry>,
    added: Query<(Entity, &MobPose), Added<GameMob>>,
) {
    let _perf = perf_debug!();
    for (entity, pose) in &added {
        registry.insert(pose.scheme.clone(), entity);
    }
}

/// Drops despawned mobs from the scheme index.
fn unindex_removed_mobs(
    mut registry: ResMut<MobRegistry>,
    mut removed: RemovedComponents<GameMob>,
) {
    let _perf = perf_debug!();
    for entity in removed.read() {
        registry.remove_entity(entity);
    }
}

/// Releases mob points held by despawned npcs.
fn release_despawned_mob_uses(
    mut commands: Commands,
    mut removed: RemovedComponents<NpcMobUse>,
    mobs: Query<(Entity, &MobOccupant)>,
) {
    let _perf = perf_debug!();
    for npc in removed.read() {
        for (mob, occupant) in &mobs {
            if occupant.0 == npc {
                commands.entity(mob).remove::<MobOccupant>();
            }
        }
    }
}

/// Mirrors the used mob onto the npc body state reported to the scripts.
fn sync_npc_body_state(
    gothic_script: Option<Res<ScriptResource>>,
    added: Query<(&GameNpc, &NpcMobUse), Added<NpcMobUse>>,
    mut removed: RemovedComponents<NpcMobUse>,
    npcs: Query<(&GameNpc, Option<&NpcMobUse>, Option<&NpcItemUse>)>,
) {
    let perf = perf_debug!();
    let Some(gothic_script) = gothic_script else {
        return;
    };
    let mut script = gothic_script.lock();
    for entity in removed.read() {
        perf.increment_tag("removed", 1);
        // Despawned npcs keep no script state, and a switched mob still has its
        // component here and is handled below.
        let Ok((npc, current, item_use)) = npcs.get(entity) else {
            perf.increment_tag("removed_missing", 1);
            continue;
        };
        if current.is_some() {
            perf.increment_tag("removed_switched", 1);
            continue;
        }
        // An item pose keeps its body state; only a plain npc returns to stand.
        if item_use.is_some() {
            perf.increment_tag("removed_item_use", 1);
            script.set_npc_body_state(npc.instance_id(), Some(BS_ITEMINTERACT));
            continue;
        }
        debug!("npc({}) body state cleared", npc.instance_id());
        script.set_npc_body_state(npc.instance_id(), None);
    }
    for (npc, mob_use) in &added {
        perf.increment_tag("added", 1);
        let body_state = script.scheme_body_state(mob_use.scheme.as_str());
        debug!(
            "npc({}) body state({body_state}) for mob({})",
            npc.instance_id(),
            mob_use.scheme
        );
        script.set_npc_body_state(npc.instance_id(), Some(body_state));
    }
}

/// Applies the `Wld_SetMobRoutine` states of the current time to the nearby mobs.
#[allow(clippy::type_complexity)]
fn apply_mob_routines(
    gothic_script: Option<Res<ScriptResource>>,
    game_time: Res<GameTime>,
    mut mobs: Query<(&mut MobPose, &MobBase), (With<GameMob>, With<DistanceNear>)>,
    newly_near: Query<(), (With<GameMob>, Added<DistanceNear>)>,
    mut last_minutes: Local<Option<u32>>,
) {
    let perf = perf_debug!();
    let Some(gothic_script) = gothic_script else {
        return;
    };
    let minutes = game_time.minutes_of_day();
    // The schedule only changes per in-game minute. Mobs that just came near
    // still need the current state applied even when the minute is unchanged.
    if *last_minutes == Some(minutes) && newly_near.is_empty() {
        return;
    }
    *last_minutes = Some(minutes);
    let routines = {
        let script = gothic_script.lock();
        let routines = &script.get_loaded_state().mob_routines;
        if routines.is_empty() {
            return;
        }
        routines.clone()
    };
    perf.increment_tag("routines", routines.len() as u64);
    let mut states: Vec<(String, i32)> = Vec::new();
    for routine in &routines {
        if states
            .iter()
            .any(|(scheme, _)| scheme.eq_ignore_ascii_case(&routine.mob_name))
        {
            continue;
        }
        states.push((routine.mob_name.clone(), 0));
    }
    for (scheme, state) in &mut states {
        *state = routine_state_at(&routines, scheme, minutes).unwrap_or(0);
    }
    perf.increment_tag("states", states.len() as u64);
    for (mut pose, base) in &mut mobs {
        perf.increment_tag("mobs", 1);
        // Mobs without a schedule keep the state the interaction gave them.
        let Some((_, state)) = states
            .iter()
            .find(|(scheme, _)| scheme.eq_ignore_ascii_case(&pose.scheme))
        else {
            continue;
        };
        if pose.state == *state {
            continue;
        }
        perf.increment_tag("changed", 1);
        debug!(
            "mob({}) scheme({}) routine state {} -> {state}",
            base.focus_name, pose.scheme, pose.state
        );
        pose.state = *state;
    }
}

/// State of the last schedule entry at or before `minutes`, the last entry when
/// every entry is still ahead, matching the engine `MobStates` semantics.
fn routine_state_at(routines: &[MobRoutineEntry], scheme: &str, minutes: u32) -> Option<i32> {
    let mut active: Option<(u32, i32)> = None;
    let mut latest: Option<(u32, i32)> = None;
    for routine in routines {
        if !routine.mob_name.eq_ignore_ascii_case(scheme) {
            continue;
        }
        let at = (routine.hour.max(0) as u32) * 60 + routine.minute.max(0) as u32;
        if latest.is_none_or(|(latest_at, _)| at >= latest_at) {
            latest = Some((at, routine.state));
        }
        if at <= minutes && active.is_none_or(|(active_at, _)| at >= active_at) {
            active = Some((at, routine.state));
        }
    }
    active.or(latest).map(|(_, state)| state)
}

/// Animation of a mob in `state`, `None` when the mob has no such animation file.
pub(crate) fn mob_state_animation(
    files: &AnimationFiles,
    handles: &mut CachedAssetHandles,
    asset_server: &AssetServer,
    stem: &str,
    state: i32,
) -> Option<Handle<AnimationData>> {
    let animation = format!("S_S{state}");
    if !files.0.contains(&animation_file_name(stem, &animation)) {
        return None;
    }
    Some(handles.get_animation_handle(asset_server, &get_animation_path(stem, &animation)))
}

/// Transition `<STEM>-T_S<from>_2_S<to>.MAN`, if the archive has it.
fn mob_transition_animation(
    files: &AnimationFiles,
    handles: &mut CachedAssetHandles,
    asset_server: &AssetServer,
    stem: &str,
    from: i32,
    to: i32,
) -> Option<(String, Handle<AnimationData>)> {
    let animation = format!("T_S{from}_2_S{to}");
    if !files.0.contains(&animation_file_name(stem, &animation)) {
        return None;
    }
    let handle = handles.get_animation_handle(asset_server, &get_animation_path(stem, &animation));
    Some((animation, handle))
}

/// Plays the state transition then the `<STEM>-S_S<n>.MAN` loop of the mob state.
#[allow(clippy::type_complexity, clippy::cognitive_complexity)]
fn apply_mob_animations(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    animation_files: Option<Res<AnimationFiles>>,
    mut handles: ResMut<CachedAssetHandles>,
    mut mobs: Query<(
        Entity,
        &MobPose,
        &MobBase,
        &mut MobAnimation,
        &mut ZenGinModelComponent,
        Option<&mut AnimationController>,
    )>,
) {
    let _perf = perf_debug!();
    let Some(animation_files) = animation_files else {
        return;
    };
    for (entity, pose, base, mut animation, mut component, controller) in &mut mobs {
        if animation.state == pose.state {
            continue;
        }
        let from = animation.state;
        let state = pose.state;
        animation.state = state;

        if controller.is_some()
            && let Some((transition, handle)) = mob_transition_animation(
                &animation_files,
                &mut handles,
                &asset_server,
                &animation.stem,
                from,
                state,
            )
        {
            component.animation_handle = Some(handle.clone());
            if let Some(mut controller) = controller {
                controller.queue_next(handle, true);
                commands.entity(entity).insert(MobStateTransition {
                    expected: transition.clone(),
                    to: state,
                });
            }
            debug!(
                "mob({}) transition({transition}) stem({})",
                base.focus_name, animation.stem
            );
            continue;
        }

        commands.entity(entity).remove::<MobStateTransition>();
        let Some(handle) = mob_state_animation(
            &animation_files,
            &mut handles,
            &asset_server,
            &animation.stem,
            state,
        ) else {
            debug!(
                "mob({}) has no animation for state({state})",
                base.focus_name
            );
            continue;
        };
        component.animation_handle = Some(handle.clone());
        if let Some(mut controller) = controller {
            controller.queue_next(handle, true);
        }
        debug!(
            "mob({}) animation state({state}) stem({})",
            base.focus_name, animation.stem
        );
    }
}

/// Queues the state loop once the transition of a mob state change ends.
fn advance_mob_state_transition(
    mut commands: Commands,
    mut ends: MessageReader<AnimationEnds>,
    files: Res<AnimationFiles>,
    mut handles: ResMut<CachedAssetHandles>,
    asset_server: Res<AssetServer>,
    mut mobs: Query<(
        &MobAnimation,
        &MobStateTransition,
        Option<&mut AnimationController>,
    )>,
) {
    let _perf = perf_debug!();
    for end in ends.read() {
        let Ok((animation, transition, controller)) = mobs.get_mut(end.who) else {
            continue;
        };
        let Some(path) = Handle::path(&end.what_animation) else {
            continue;
        };
        let Some(ended) = animation_name_from_path(path.path()) else {
            continue;
        };
        if ended != transition.expected {
            continue;
        }
        if let Some(mut controller) = controller
            && let Some(handle) = mob_state_animation(
                &files,
                &mut handles,
                &asset_server,
                &animation.stem,
                transition.to,
            )
        {
            controller.queue_next(handle, true);
        }
        commands.entity(end.who).remove::<MobStateTransition>();
    }
}

/// Publishes the mob table for `Wld_IsMobAvailable`/`Wld_GetMobState`.
fn sync_script_mobs(
    gothic_script: Option<Res<ScriptResource>>,
    mobs: Query<(&MobPose, &GlobalTransform, Option<&MobOccupant>), With<GameMob>>,
    mut initialized: Local<bool>,
    added_occupancy: Query<(), Added<MobOccupant>>,
    changed_mobs: Query<(), Changed<MobPose>>,
    mut removed_occupancy: RemovedComponents<MobOccupant>,
) {
    let _perf = perf_debug!();
    let occupancy_removed = removed_occupancy.read().count() > 0;
    let dirty = !*initialized
        || occupancy_removed
        || !added_occupancy.is_empty()
        || !changed_mobs.is_empty();
    if !dirty {
        return;
    }
    let Some(gothic_script) = gothic_script else {
        return;
    };
    let snapshot = mobs
        .iter()
        .map(|(pose, mob_tr, occupant)| {
            (
                pose.scheme.to_lowercase(),
                ScriptMob {
                    position: mob_tr.translation(),
                    occupied: occupant.is_some(),
                    state: pose.state,
                },
            )
        })
        .collect();
    gothic_script.lock().set_mobs(snapshot);
    *initialized = true;
}

/// Gives a freshly built mob visual a selection collider sized to its model
/// bounds. The collider is a child so it follows the vob transform and is
/// respawned with the visual.
#[allow(clippy::type_complexity)]
fn add_mob_selection_colliders(
    mut commands: Commands,
    mobs: Query<
        MobVisualQuery,
        (
            Or<(With<GameMob>, With<GameChest>)>,
            Added<ZenGinModelComponentLoaded>,
        ),
    >,
) {
    let _perf = perf_debug!();
    for (entity, aabb) in &mobs {
        let half = (aabb.max() - aabb.min()) * 0.5;
        commands.entity(entity).with_child((
            Collider::cuboid(
                half.x.max(MIN_MOB_COLLIDER_HALF_EXTENT),
                half.y.max(MIN_MOB_COLLIDER_HALF_EXTENT),
                half.z.max(MIN_MOB_COLLIDER_HALF_EXTENT),
            ),
            CollisionLayers::from_bits(SELECTABLE, 0),
            HitColliderOwner { owner: entity },
            Transform::from_translation(Vec3::from(aabb.center)),
        ));
    }
}

/// Fills [`MobPose::positions`] once the vob model asset is loaded.
fn extract_mob_positions(
    mut mobs: Query<(
        &mut MobPose,
        &MobBase,
        Option<&ZenGinModelComponent>,
        Option<&MobHiddenModel>,
    )>,
    models: Res<Assets<ZenGinModel>>,
) {
    let _perf = perf_debug!();
    for (mut pose, base, model_component, hidden_model) in &mut mobs {
        if pose.positions.is_some() {
            continue;
        }
        let model_handle = model_component
            .map(|component| &component.model_handle)
            .or_else(|| hidden_model.map(|hidden| &hidden.0));
        let Some(model) = model_handle.and_then(|handle| models.get(handle)) else {
            continue;
        };
        let positions = mob_positions(model);
        pose.rendered_origin = if model.has_skinned_meshes {
            model.root_translation
        } else {
            Vec3::ZERO
        };
        debug!(
            "mob({}) extracted {} attachment point(s)",
            base.focus_name,
            positions.len()
        );
        pose.positions = Some(positions);
    }
}

/// `ZS_POS*` nodes of a mob model with their model-space transforms.
///
/// The hierarchy root translation is a bind-pose offset, not part of the node
/// space the engine positions actors in: the attachment nodes are taken from
/// the raw node chain. The visible mesh cancels the offset too (skinned joints
/// use the raw chain while their bind poses carry it), so keeping it would move
/// the attachment point by the root offset relative to the mesh. For static
/// attachment-only models (the hidden `REPAIR_PLANK`, whose geometry has no
/// texture and is not rendered) it would also lift the actor off the ground by
/// the whole root height.
fn mob_positions(model: &ZenGinModel) -> Vec<crate::zengin::MobPosition> {
    model
        .node_names
        .iter()
        .zip(&model.final_tr)
        .filter(|(name, _)| name.starts_with("ZS_POS"))
        .map(|(name, transform)| {
            let mut mob_transform = *transform;
            mob_transform.translation -= model.root_translation;
            crate::zengin::MobPosition {
                name: name.clone(),
                transform: mob_transform,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mob_positions_keep_zs_pos_nodes_in_model_space() {
        let front = Transform::from_xyz(1.0, 2.0, 3.0);
        let model = ZenGinModel {
            node_names: vec!["BIP01".to_string(), "ZS_POS0_FRONT".to_string()],
            final_tr: vec![Transform::IDENTITY, front],
            ..Default::default()
        };

        let positions = mob_positions(&model);

        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0].name, "ZS_POS0_FRONT");
        assert_eq!(positions[0].transform, front);
    }

    #[test]
    fn mob_positions_remove_the_hierarchy_root_translation() {
        let root = Vec3::new(0.0, 0.0, 0.75);
        let model = ZenGinModel {
            node_names: vec!["BIP01".to_string(), "ZS_POS0".to_string()],
            final_tr: vec![Transform::IDENTITY, Transform::from_xyz(0.0, 0.977, 0.824)],
            root_translation: root,
            has_skinned_meshes: true,
            ..Default::default()
        };

        let positions = mob_positions(&model);

        assert_eq!(positions.len(), 1);
        assert_eq!(
            positions[0].transform.translation,
            Vec3::new(0.0, 0.977, 0.074)
        );
    }

    #[test]
    fn mob_positions_remove_the_root_translation_without_rendered_meshes() {
        // The hidden `REPAIR_PLANK` only ships a textureless attachment, so the
        // model has no skinned meshes; the bind-pose offset must be removed all
        // the same, or the actor floats one root height above the ground.
        let root = Vec3::new(0.0, 1.336, 0.55);
        let model = ZenGinModel {
            node_names: vec!["ZM_BOOK1".to_string(), "ZS_POS0".to_string()],
            final_tr: vec![Transform::IDENTITY, Transform::from_xyz(0.0, 0.971, -0.021)],
            root_translation: root,
            ..Default::default()
        };

        let positions = mob_positions(&model);

        assert_eq!(positions.len(), 1);
        let expected = Vec3::new(0.0, 0.971 - 1.336, -0.021 - 0.55);
        assert!(
            (positions[0].transform.translation - expected).length() < 1e-5,
            "got {:?}, expected {expected:?}",
            positions[0].transform.translation
        );
    }

    #[test]
    fn routine_state_uses_the_last_entry_at_or_before_the_minute() {
        let routines = [routine(5, 0, 0), routine(0, 0, 1), routine(20, 0, 1)];

        assert_eq!(routine_state_at(&routines, "FIREPLACE", 4 * 60), Some(1));
        assert_eq!(routine_state_at(&routines, "FIREPLACE", 10 * 60), Some(0));
        assert_eq!(routine_state_at(&routines, "fireplace", 21 * 60), Some(1));
    }

    #[test]
    fn mob_visual_gets_a_selection_collider() {
        let mut app = App::new();
        app.add_systems(Update, add_mob_selection_colliders);
        let (mob_component, pose) = test_mob("DOOR", "door_nw_normal_01");
        let mob = app
            .world_mut()
            .spawn((
                mob_component,
                pose,
                MobBase::default(),
                Aabb::from_min_max(Vec3::new(-1.0, 0.0, -1.0), Vec3::new(1.0, 2.0, 1.0)),
                ZenGinModelComponentLoaded::default(),
            ))
            .id();

        app.update();

        let children = app.world().get::<Children>(mob).expect("collider child");
        assert_eq!(children.len(), 1);
        let collider = children[0];
        assert!(app.world().get::<Collider>(collider).is_some());
        // Selection resolves a collider hit back to the entity it belongs to.
        let owner = app
            .world()
            .get::<HitColliderOwner>(collider)
            .expect("collider owner");
        assert_eq!(owner.owner, mob);
    }

    fn test_mob(scheme: &str, stem: &str) -> (GameMob, MobPose) {
        (
            GameMob {
                use_with_item: String::new(),
                on_state_func: String::new(),
                condition_func: String::new(),
                rewind: false,
                trigger_target: String::new(),
                trigger_fired: false,
                auto_end: false,
            },
            MobPose {
                scheme: scheme.to_string(),
                visual_stem: stem.to_string(),
                state_num: 1,
                ..default()
            },
        )
    }

    fn routine(hour: i32, minute: i32, state: i32) -> MobRoutineEntry {
        MobRoutineEntry {
            hour,
            minute,
            mob_name: "FIREPLACE".to_string(),
            state,
        }
    }
}
