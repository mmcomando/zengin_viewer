//! Interactive mob registry and mesh attachment-point extraction.

use std::collections::HashMap;

use avian3d::prelude::{Collider, CollisionLayers};
use bevy::camera::primitives::Aabb;
use bevy::prelude::*;

use crate::gameplay::game_time::GameTime;
use crate::graphics::{AnimationController, AnimationData};
use crate::zengin::GameMob;
use crate::zengin::GameNpc;
use crate::zengin::common::{animation_file_name, get_animation_path};
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
}

/// Npc entity occupying a mob interaction point.
#[derive(Component, Debug, Clone, Copy)]
pub struct MobOccupant(pub Entity);

/// Animation currently applied to a mob for its state.
#[derive(Component, Debug, Clone)]
pub struct MobAnimation {
    /// Visual file stem, the `<STEM>` of the `<STEM>-S_S<n>.MAN` set.
    pub stem: String,
    /// State the animation handle of the model belongs to.
    pub state: i32,
}

/// Registers freshly spawned mobs under their `AI_UseMob` scheme.
fn index_added_mobs(
    mut registry: ResMut<MobRegistry>,
    added: Query<(Entity, &GameMob), Added<GameMob>>,
) {
    for (entity, mob) in &added {
        registry.insert(mob.scheme.clone(), entity);
    }
}

/// Drops despawned mobs from the scheme index.
fn unindex_removed_mobs(
    mut registry: ResMut<MobRegistry>,
    mut removed: RemovedComponents<GameMob>,
) {
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
    let Some(gothic_script) = gothic_script else {
        return;
    };
    let mut script = gothic_script.lock();
    for entity in removed.read() {
        // Despawned npcs keep no script state, and a switched mob still has its
        // component here and is handled below.
        let Ok((npc, current, item_use)) = npcs.get(entity) else {
            continue;
        };
        if current.is_some() {
            continue;
        }
        // An item pose keeps its body state; only a plain npc returns to stand.
        if item_use.is_some() {
            script.set_npc_body_state(npc.instance_id(), Some(BS_ITEMINTERACT));
            continue;
        }
        debug!("npc({}) body state cleared", npc.instance_id());
        script.set_npc_body_state(npc.instance_id(), None);
    }
    for (npc, mob_use) in &added {
        let body_state = script.scheme_body_state(mob_use.scheme.as_str());
        debug!(
            "npc({}) body state({body_state}) for mob({})",
            npc.instance_id(),
            mob_use.scheme
        );
        script.set_npc_body_state(npc.instance_id(), Some(body_state));
    }
}

/// Applies the `Wld_SetMobRoutine` states of the current time to the mobs.
fn apply_mob_routines(
    gothic_script: Option<Res<ScriptResource>>,
    game_time: Res<GameTime>,
    mut mobs: Query<&mut GameMob>,
) {
    let Some(gothic_script) = gothic_script else {
        return;
    };
    let routines = {
        let script = gothic_script.lock();
        let routines = &script.get_loaded_state().mob_routines;
        if routines.is_empty() {
            return;
        }
        routines.clone()
    };
    let minutes = game_time.minutes_of_day();
    let mut states: HashMap<String, i32> = HashMap::new();
    for routine in &routines {
        states.entry(routine.mob_name.to_uppercase()).or_insert(0);
    }
    for (scheme, state) in &mut states {
        *state = routine_state_at(&routines, scheme, minutes).unwrap_or(0);
    }
    for mut mob in &mut mobs {
        let scheme = mob.scheme.to_uppercase();
        // Mobs without a schedule keep the state the interaction gave them.
        let Some(state) = states.get(&scheme) else {
            continue;
        };
        if mob.state == *state {
            continue;
        }
        debug!(
            "mob({}) scheme({scheme}) routine state {} -> {state}",
            mob.focus_name, mob.state
        );
        mob.state = *state;
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

/// Plays the `<STEM>-S_S<n>.MAN` animation of the mob state on the mob itself.
fn apply_mob_animations(
    asset_server: Res<AssetServer>,
    animation_files: Option<Res<AnimationFiles>>,
    mut handles: ResMut<CachedAssetHandles>,
    mut mobs: Query<(
        &GameMob,
        &mut MobAnimation,
        &mut ZenGinModelComponent,
        Option<&mut AnimationController>,
    )>,
) {
    let Some(animation_files) = animation_files else {
        return;
    };
    for (mob, mut animation, mut component, controller) in &mut mobs {
        if animation.state == mob.state {
            continue;
        }
        let state = mob.state;
        let Some(handle) = mob_state_animation(
            &animation_files,
            &mut handles,
            &asset_server,
            &animation.stem,
            state,
        ) else {
            debug!(
                "mob({}) has no animation for state({state})",
                mob.focus_name
            );
            animation.state = state;
            continue;
        };
        animation.state = state;
        component.animation_handle = Some(handle.clone());
        if let Some(mut controller) = controller {
            controller.queue_next(handle, true);
        }
        debug!(
            "mob({}) animation state({state}) stem({})",
            mob.focus_name, animation.stem
        );
    }
}

/// Publishes the mob table for `Wld_IsMobAvailable`/`Wld_GetMobState`.
fn sync_script_mobs(
    gothic_script: Option<Res<ScriptResource>>,
    mobs: Query<(&GameMob, &GlobalTransform, Option<&MobOccupant>)>,
    mut initialized: Local<bool>,
    added_occupancy: Query<(), Added<MobOccupant>>,
    changed_mobs: Query<(), Changed<GameMob>>,
    mut removed_occupancy: RemovedComponents<MobOccupant>,
) {
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
        .map(|(mob, mob_tr, occupant)| {
            (
                mob.scheme.to_lowercase(),
                ScriptMob {
                    position: mob_tr.translation(),
                    occupied: occupant.is_some(),
                    state: mob.state,
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
fn add_mob_selection_colliders(
    mut commands: Commands,
    mobs: Query<MobVisualQuery, (With<GameMob>, Added<ZenGinModelComponentLoaded>)>,
) {
    for (entity, aabb) in &mobs {
        let half = (aabb.max() - aabb.min()) * 0.5;
        commands.entity(entity).with_child((
            Collider::cuboid(
                half.x.max(MIN_MOB_COLLIDER_HALF_EXTENT),
                half.y.max(MIN_MOB_COLLIDER_HALF_EXTENT),
                half.z.max(MIN_MOB_COLLIDER_HALF_EXTENT),
            ),
            CollisionLayers::from_bits(SELECTABLE, 0),
            Transform::from_translation(Vec3::from(aabb.center)),
        ));
    }
}

/// Fills [`GameMob::positions`] once the mob model asset is loaded.
fn extract_mob_positions(
    mut mobs: Query<(&mut GameMob, &ZenGinModelComponent)>,
    models: Res<Assets<ZenGinModel>>,
) {
    for (mut mob, model_component) in &mut mobs {
        if mob.positions.is_some() {
            continue;
        }
        let Some(model) = models.get(&model_component.model_handle) else {
            continue;
        };
        let positions = mob_positions(model);
        debug!(
            "mob({}) extracted {} attachment point(s)",
            mob.focus_name,
            positions.len()
        );
        mob.positions = Some(positions);
    }
}

/// `ZS_POS*` nodes of a mob model with their model-space transforms.
///
/// Skinned meshes are rendered in raw hierarchy space: the joint chain uses
/// the raw node transforms while the bind poses carry the hierarchy root
/// translation, so the two cancel out. Attachment points therefore have to be
/// taken without `root_translation` too, otherwise they are shifted relative
/// to the visible mesh by that offset.
fn mob_positions(model: &ZenGinModel) -> Vec<crate::zengin::MobPosition> {
    model
        .node_names
        .iter()
        .zip(&model.final_tr)
        .filter(|(name, _)| name.starts_with("ZS_POS"))
        .map(|(name, transform)| {
            let mut transform = *transform;
            transform.translation -= model.root_translation;
            crate::zengin::MobPosition {
                name: name.clone(),
                transform,
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
        let mob = app
            .world_mut()
            .spawn((
                test_mob("DOOR", "door_nw_normal_01"),
                Aabb::from_min_max(Vec3::new(-1.0, 0.0, -1.0), Vec3::new(1.0, 2.0, 1.0)),
                ZenGinModelComponentLoaded::default(),
            ))
            .id();

        app.update();

        let children = app.world().get::<Children>(mob).expect("collider child");
        assert_eq!(children.len(), 1);
        assert!(app.world().get::<Collider>(children[0]).is_some());
    }

    fn test_mob(scheme: &str, stem: &str) -> GameMob {
        GameMob {
            vob_name: String::new(),
            focus_name: String::new(),
            scheme: scheme.to_string(),
            visual_stem: stem.to_string(),
            state_num: 1,
            use_with_item: String::new(),
            on_state_func: String::new(),
            condition_func: String::new(),
            rewind: false,
            trigger_target: String::new(),
            trigger_fired: false,
            auto_end: false,
            no_move: false,
            positions: None,
            state: 0,
        }
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
