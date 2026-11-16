//! Spawn messages for the entities produced by the world Zen load: the world
//! mesh, static vob models, chests and interactive mobs. The loader queues them
//! and the systems here turn them into ECS entities.

use avian3d::prelude::{Collider, CollisionLayers};
use bevy::prelude::*;

use crate::external_messages::{ExternalMessageAppExt, ExternalMessageSender};
use crate::game_state::AppState;
use crate::gameplay::distance::ClassifyDistance;
use crate::gui_debug::{StaticMesh, WorldMesh};
use crate::zengin::mobs::{MobAnimation, mob_state_animation};
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
            items: app.add_external_message::<SpawnItem>(),
        };
        app.insert_resource(messages);
        app.add_systems(
            Update,
            (
                spawn_world_mesh,
                spawn_static_models,
                spawn_chests,
                spawn_mobs,
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
}

/// Queue handles the world load uses to spawn its entities.
#[derive(Resource)]
pub(crate) struct WorldSpawnMessages {
    pub(crate) world_mesh: ExternalMessageSender<SpawnWorldMesh>,
    pub(crate) static_models: ExternalMessageSender<SpawnStaticModel>,
    pub(crate) chests: ExternalMessageSender<SpawnChest>,
    pub(crate) mobs: ExternalMessageSender<SpawnMob>,
    pub(crate) items: ExternalMessageSender<SpawnItem>,
}

impl WorldSpawnMessages {
    /// Drops the spawns a world load queued but the pump has not forwarded yet.
    pub(crate) fn clear(&self) {
        self.world_mesh.clear();
        self.static_models.clear();
        self.chests.clear();
        self.mobs.clear();
        self.items.clear();
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
) {
    for spawn in spawns.read() {
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
) {
    for spawn in spawns.read() {
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
) {
    let mobs: Vec<SpawnMob> = spawns.read().cloned().collect();
    if mobs.is_empty() {
        return;
    }
    for mob in mobs {
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
