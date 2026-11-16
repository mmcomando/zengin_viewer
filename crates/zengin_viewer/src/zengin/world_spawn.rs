//! Spawn messages for the entities produced by the world Zen load: the world
//! mesh, static vob models, chests and interactive mobs. The loader queues them
//! and the systems here turn them into ECS entities.

use avian3d::prelude::{Collider, CollisionLayers};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::external_messages::{ExternalMessageAppExt, ExternalMessageSender};
use crate::game_state::AppState;
use crate::gameplay::distance::ClassifyDistance;
use crate::gui_debug::{StaticMesh, WorldMesh};
use crate::zengin::mobs::{MobAnimation, mob_state_animation};
use crate::zengin::movers::GameMover;
use crate::zengin::npc_routines::AnimationFiles;
use crate::zengin::resources::{CachedAssetHandles, SELECTABLE, ZenGinModelComponent};
use crate::zengin::script::bridge::ScriptChangesApplied;
use crate::zengin::script::component::ScriptResource;
use crate::zengin::script::state::SpawnItem;
use crate::zengin::script::symbol_id::InstanceId;
use crate::zengin::visual::model::ZenGinModel;
use crate::zengin::{GameChest, GameMob};

pub struct WorldSpawnPlugin;

impl Plugin for WorldSpawnPlugin {
    fn build(&self, app: &mut App) {
        let messages = WorldSpawnMessages {
            world_mesh: app.add_external_message::<SpawnWorldMesh>(),
            static_models: app.add_external_message::<SpawnStaticModel>(),
            chests: app.add_external_message::<SpawnChest>(),
            mobs: app.add_external_message::<SpawnMob>(),
            movers: app.add_external_message::<SpawnMover>(),
            items: app.add_external_message::<SpawnItem>(),
        };
        app.insert_resource(messages);
        app.init_resource::<SpawnPositionFilter>();
        app.add_systems(
            Update,
            (
                spawn_world_mesh,
                spawn_static_models,
                spawn_chests,
                spawn_mobs,
                spawn_movers,
            )
                .chain()
                .after(ScriptChangesApplied)
                .run_if(resource_exists::<ScriptResource>)
                .run_if(in_state(AppState::LoadScreen).or_else(in_state(AppState::InGame))),
        );
    }
}

/// World mesh geometry of the freshly loaded world.
#[derive(Debug, Message)]
pub(crate) struct SpawnWorldMesh {
    pub(crate) model: ZenGinModel,
}

/// Static vob visual placed at its world transform.
#[derive(Debug, Default, Message)]
pub(crate) struct SpawnStaticModel {
    pub(crate) tr: Transform,
    pub(crate) archetype: String,
    pub(crate) collides: bool,
}

/// Interactive chest vob with its remaining loot.
#[derive(Debug, Default, Clone, Message)]
pub(crate) struct SpawnChest {
    /// Index of the vob in the ZEN archive, stable across loads.
    pub(crate) vob_id: u32,
    pub(crate) tr: Transform,
    /// Mesh asset path, empty when the vob has no visual.
    pub(crate) archetype: String,
    pub(crate) collides: bool,
    /// Half-size of the selection collider, converted to world units.
    pub(crate) collider_half_extents: Vec3,
    pub(crate) name: String,
    pub(crate) items: Vec<(InstanceId, i32)>,
    pub(crate) locked: bool,
}

/// Interactive mob vob (`oCMobInter` and its subtypes).
#[derive(Debug, Default, Clone, Message)]
pub(crate) struct SpawnMob {
    pub(crate) tr: Transform,
    /// Mesh asset path, empty when the vob has no visual.
    pub(crate) archetype: String,
    pub(crate) collides: bool,
    /// Vob name, used by scripts and trigger targets.
    pub(crate) vob_name: String,
    /// In-game focus name (`oCMOB.name`), shown when the player looks at it.
    pub(crate) focus_name: String,
    /// `AI_UseMob` scheme derived from the visual stem.
    pub(crate) scheme: String,
    /// Visual file stem, the `<STEM>` of the mob's own animation set.
    pub(crate) visual_stem: String,
    /// Number of interaction states (`stateNum`).
    pub(crate) state_num: i32,
    /// Script item instance required to use the mob, empty when none.
    pub(crate) use_with_item: String,
    /// Script function called when the mob changes state, empty when none.
    pub(crate) on_state_func: String,
    /// Script condition gating use of the mob, empty when none.
    pub(crate) condition_func: String,
    pub(crate) rewind: bool,
    /// Script name of the vob notified (`triggerTarget`) when the mob is used.
    pub(crate) trigger_target: String,
}

/// Moving trigger vob (`zCMover`).
#[derive(Debug, Default, Clone, Message)]
pub(crate) struct SpawnMover {
    pub(crate) tr: Transform,
    /// Mesh asset path, empty when the vob has no visual.
    pub(crate) archetype: String,
    pub(crate) collides: bool,
    /// Vob name the trigger events are addressed to.
    pub(crate) name: String,
    /// Absolute keyframe poses, keyframe 0 first.
    pub(crate) keyframes: Vec<Transform>,
    /// Movement speed in meters per second.
    pub(crate) speed: f32,
    pub(crate) start_enabled: bool,
}

/// Queue handles the world load uses to spawn its entities.
#[derive(Resource)]
pub(crate) struct WorldSpawnMessages {
    pub(crate) world_mesh: ExternalMessageSender<SpawnWorldMesh>,
    pub(crate) static_models: ExternalMessageSender<SpawnStaticModel>,
    pub(crate) chests: ExternalMessageSender<SpawnChest>,
    pub(crate) mobs: ExternalMessageSender<SpawnMob>,
    pub(crate) movers: ExternalMessageSender<SpawnMover>,
    pub(crate) items: ExternalMessageSender<SpawnItem>,
}

impl WorldSpawnMessages {
    /// Drops the spawns a world load queued but the pump has not forwarded yet.
    pub(crate) fn clear(&self) {
        self.world_mesh.clear();
        self.static_models.clear();
        self.chests.clear();
        self.mobs.clear();
        self.movers.clear();
        self.items.clear();
    }
}

/// Circle in world space (meters) used by [`SpawnPositionFilter`].
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub(crate) struct SpawnPoint {
    pub(crate) center: Vec3,
    /// Radius in world units.
    pub(crate) radius: f32,
}

/// Restricts world content to the union of [`SpawnPoint`] circles. An empty
/// list allows every position.
#[derive(Resource, Default)]
pub(crate) struct SpawnPositionFilter(Vec<SpawnPoint>);

impl SpawnPositionFilter {
    pub(crate) fn add(&mut self, points: &[SpawnPoint]) {
        self.0.extend_from_slice(points);
    }

    pub(crate) fn allows(&self, position: Vec3) -> bool {
        if self.0.is_empty() {
            return true;
        }
        self.0
            .iter()
            .any(|point| position.distance_squared(point.center) <= point.radius * point.radius)
    }
}

fn spawn_world_mesh(
    mut spawns: MessageMutator<SpawnWorldMesh>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    for spawn in spawns.read() {
        let model = std::mem::take(&mut spawn.model);
        commands.spawn((
            Visibility::default(),
            ClassifyDistance::default(),
            ZenGinModelComponent {
                model_handle: asset_server.add(model),
                trimesh_collider: true,
                ..default()
            },
            Transform::IDENTITY,
            WorldMesh::default(),
        ));
    }
}

fn spawn_static_models(
    mut spawns: MessageReader<SpawnStaticModel>,
    mut commands: Commands,
    mut handles_map: ResMut<CachedAssetHandles>,
    asset_server: Res<AssetServer>,
    position_filter: Res<SpawnPositionFilter>,
) {
    for spawn in spawns.read() {
        if !position_filter.allows(spawn.tr.translation) {
            continue;
        }
        let model_handle = handles_map.get_model_handle(&asset_server, &spawn.archetype, None);
        commands.spawn((
            ClassifyDistance::default(),
            ZenGinModelComponent {
                model_handle,
                convex_colider: spawn.collides,
                ..default()
            },
            Visibility::default(),
            spawn.tr,
            StaticMesh::default(),
        ));
    }
}

fn spawn_chests(
    mut spawns: MessageReader<SpawnChest>,
    mut commands: Commands,
    mut handles_map: ResMut<CachedAssetHandles>,
    asset_server: Res<AssetServer>,
    position_filter: Res<SpawnPositionFilter>,
) {
    for spawn in spawns.read() {
        if !position_filter.allows(spawn.tr.translation) {
            continue;
        }
        let model_handle = handles_map.get_model_handle(&asset_server, &spawn.archetype, None);
        commands.spawn((
            ClassifyDistance::default(),
            GameChest {
                vob_id: spawn.vob_id,
                name: spawn.name.clone(),
                locked: spawn.locked,
                items: spawn.items.clone(),
            },
            Collider::cuboid(
                spawn.collider_half_extents.x * 2.0,
                spawn.collider_half_extents.y * 2.0,
                spawn.collider_half_extents.z * 2.0,
            ),
            CollisionLayers::from_bits(SELECTABLE, 0),
            Visibility::default(),
            spawn.tr,
            ZenGinModelComponent {
                model_handle,
                convex_colider: spawn.collides,
                ..default()
            },
        ));
    }
}

fn spawn_mobs(
    mut spawns: MessageReader<SpawnMob>,
    mut commands: Commands,
    mut handles_map: ResMut<CachedAssetHandles>,
    asset_server: Res<AssetServer>,
    animation_files: Res<AnimationFiles>,
    position_filter: Res<SpawnPositionFilter>,
) {
    let mobs: Vec<SpawnMob> = spawns.read().cloned().collect();
    if mobs.is_empty() {
        return;
    }
    for mob in mobs {
        if !position_filter.allows(mob.tr.translation) {
            continue;
        }
        let model_handle = (!mob.archetype.is_empty())
            .then(|| handles_map.get_model_handle(&asset_server, &mob.archetype, None));
        let mut entity = commands.spawn((
            ClassifyDistance::default(),
            GameMob {
                vob_name: mob.vob_name,
                focus_name: mob.focus_name,
                scheme: mob.scheme,
                visual_stem: mob.visual_stem.clone(),
                state_num: mob.state_num,
                use_with_item: mob.use_with_item,
                on_state_func: mob.on_state_func,
                condition_func: mob.condition_func,
                rewind: mob.rewind,
                trigger_target: mob.trigger_target,
                trigger_fired: false,
                positions: None,
                state: 0,
            },
            Visibility::default(),
            Collider::sphere(0.5),
            CollisionLayers::from_bits(SELECTABLE, 0),
            mob.tr,
        ));
        if let Some(model_handle) = model_handle {
            let animation_handle = mob_state_animation(
                &animation_files,
                &mut handles_map,
                &asset_server,
                &mob.visual_stem,
                0,
            );
            entity.insert((
                ZenGinModelComponent {
                    model_handle,
                    convex_colider: mob.collides,
                    animation_handle,
                    ..default()
                },
                MobAnimation {
                    stem: mob.visual_stem.clone(),
                    state: 0,
                },
            ));
        }
    }
}

fn spawn_movers(
    mut spawns: MessageReader<SpawnMover>,
    mut commands: Commands,
    mut handles_map: ResMut<CachedAssetHandles>,
    asset_server: Res<AssetServer>,
    position_filter: Res<SpawnPositionFilter>,
) {
    for spawn in spawns.read() {
        if !position_filter.allows(spawn.tr.translation) {
            continue;
        }
        let model_handle = (!spawn.archetype.is_empty())
            .then(|| handles_map.get_model_handle(&asset_server, &spawn.archetype, None));
        let mut entity = commands.spawn((
            ClassifyDistance::default(),
            StaticMesh::default(),
            Visibility::default(),
            spawn.tr,
            GameMover::new(
                spawn.name.clone(),
                spawn.keyframes.clone(),
                spawn.tr,
                spawn.speed,
                spawn.start_enabled,
            ),
        ));
        if let Some(model_handle) = model_handle {
            entity.insert(ZenGinModelComponent {
                model_handle,
                convex_colider: spawn.collides,
                ..default()
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(center: Vec3, radius: f32) -> SpawnPoint {
        SpawnPoint { center, radius }
    }

    #[test]
    fn empty_filter_allows_every_position() {
        let filter = SpawnPositionFilter::default();
        assert!(filter.allows(Vec3::new(1000.0, 0.0, 0.0)));
    }

    #[test]
    fn position_inside_any_point_is_allowed() {
        let mut filter = SpawnPositionFilter::default();
        filter.add(&[
            point(Vec3::ZERO, 10.0),
            point(Vec3::new(100.0, 0.0, 0.0), 5.0),
        ]);

        assert!(filter.allows(Vec3::new(3.0, 0.0, 0.0)));
        assert!(filter.allows(Vec3::new(100.0, 0.0, 0.0)));
        assert!(filter.allows(Vec3::new(95.0, 0.0, 0.0)));
        assert!(!filter.allows(Vec3::new(50.0, 0.0, 0.0)));
    }

    #[test]
    fn added_points_accumulate() {
        let mut filter = SpawnPositionFilter::default();
        filter.add(&[point(Vec3::ZERO, 10.0)]);
        filter.add(&[point(Vec3::new(100.0, 0.0, 0.0), 10.0)]);

        assert!(filter.allows(Vec3::new(100.0, 0.0, 0.0)));
    }
}
