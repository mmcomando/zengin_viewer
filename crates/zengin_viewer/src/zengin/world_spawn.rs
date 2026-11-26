//! Spawn messages for the entities produced by the world Zen load: the world
//! mesh, static vob models, mob vobs (interactive and containers) and movers.
//! The loader resolves one [`VobPrototype`] per spawnable vob and keeps them in
//! [`VobPrototypes`]; the systems here turn a prototype into ECS entities.

use std::collections::{BTreeMap, HashMap};

use avian3d::prelude::{Collider, CollisionLayers};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::zengin::AppState;
use crate::zengin::gameplay::distance::ClassifyDistance;
use crate::zengin::mobs::{MobAnimation, MobHiddenModel, mob_state_animation};
use crate::zengin::movers::GameMover;
use crate::zengin::npc_routines::AnimationFiles;
use crate::zengin::resources::{CachedAssetHandles, SELECTABLE, ZenGinModelComponent};
use crate::zengin::save::format::SavedObject;
use crate::zengin::script::bridge::ScriptChangesApplied;
use crate::zengin::script::component::ScriptResource;
use crate::zengin::script::state::SpawnItem;
use crate::zengin::script::symbol_id::InstanceId;
use crate::zengin::visual::model::ZenGinModel;
use crate::zengin::{GameChest, GameMob, MobBase, MobPose};
use crate::zengin::{StaticMesh, WorldMesh};
use dev_util::external_messages::{ExternalMessageAppExt, ExternalMessageSender};
use dev_util::perf_debug;
use zen_kit_rs::misc::MoverBehavior;

pub struct WorldSpawnPlugin;

impl Plugin for WorldSpawnPlugin {
    fn build(&self, app: &mut App) {
        let messages = WorldSpawnMessages {
            world_mesh: app.add_external_message::<SpawnWorldMesh>(),
            static_models: app.add_external_message::<SpawnStaticModel>(),
            mobs: app.add_external_message::<SpawnMob>(),
            movers: app.add_external_message::<SpawnMover>(),
            items: app.add_external_message::<SpawnItem>(),
            objects: app.add_external_message::<SpawnFromSavedObject>(),
        };
        app.insert_resource(messages);
        app.init_resource::<SpawnPositionFilter>();
        app.init_resource::<VobPrototypes>();
        app.add_systems(
            Update,
            (
                spawn_world_mesh,
                spawn_static_models,
                spawn_mobs,
                sync_script_containers,
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
#[derive(Debug, Default, Clone, Message)]
pub(crate) struct SpawnStaticModel {
    /// Index of the vob in the ZEN archive.
    pub(crate) vob_id: u32,
    pub(crate) tr: Transform,
    pub(crate) archetype: String,
    pub(crate) collides: bool,
}

/// Mob vob placed at its world transform, interactive or a container.
#[derive(Debug, Clone, Message)]
pub(crate) struct SpawnMob {
    /// Fields shared by every `oCMob` vob.
    pub(crate) base: MobBase,
    pub(crate) tr: Transform,
    /// Mesh asset path, empty when the vob has no visual.
    pub(crate) archetype: String,
    /// The vob has `showVisual = false`: load the model only for its `ZS_POS*`
    /// attachment nodes, without rendering it.
    pub(crate) visual_hidden: bool,
    pub(crate) collides: bool,
    /// `AI_UseMob` / player animation scheme derived from the visual stem.
    pub(crate) scheme: String,
    /// Visual file stem, the `<STEM>` of the vob's own animation set.
    pub(crate) visual_stem: String,
    /// Number of interaction states (`stateNum`).
    pub(crate) state_num: i32,
    pub(crate) kind: MobSpawnKind,
}

/// Kind-specific data of a spawned mob vob.
#[derive(Debug, Clone)]
pub(crate) enum MobSpawnKind {
    /// `oCMobInter` family, driven by `AI_UseMob`.
    Interactive(InteractiveSpawn),
    /// `oCMobContainer` with its remaining loot.
    Container(ContainerSpawn),
}

/// Interaction data of an `oCMobInter` vob.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Default, Clone)]
pub(crate) struct InteractiveSpawn {
    /// Script item instance required to use the mob, empty when none.
    pub(crate) use_with_item: String,
    /// Script function called when the mob changes state, empty when none.
    pub(crate) on_state_func: String,
    /// Script condition gating use of the mob, empty when none.
    pub(crate) condition_func: String,
    pub(crate) rewind: bool,
    /// Script name of the vob notified (`triggerTarget`) when the mob is used.
    pub(crate) trigger_target: String,
    /// Switch-like vob: using it toggles the state and ends automatically.
    pub(crate) auto_end: bool,
    /// Door: the player uses it from where they stand instead of snapping to
    /// the attachment point.
    pub(crate) no_move: bool,
}

/// Loot and lock data of an `oCMobContainer` vob.
#[derive(Debug, Default, Clone)]
pub(crate) struct ContainerSpawn {
    pub(crate) items: Vec<(InstanceId, i32)>,
    pub(crate) locked: bool,
    /// Script instance name of the unlocking item.
    pub(crate) key: String,
    /// `R`/`L` per lockpick step, empty when the lock cannot be picked.
    pub(crate) pick_string: String,
    /// Restored from a save, `false` for a fresh chest.
    pub(crate) cracked: bool,
}

/// Moving trigger vob (`zCMover`).
#[derive(Debug, Clone, Message)]
pub(crate) struct SpawnMover {
    /// Index of the vob in the ZEN archive.
    pub(crate) vob_id: u32,
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
    /// `zCMover.moverBehavior`, decides the reaction to a trigger.
    pub(crate) behavior: MoverBehavior,
}

/// Queue handles the world load uses to spawn its entities.
#[derive(Resource)]
pub(crate) struct WorldSpawnMessages {
    pub(crate) world_mesh: ExternalMessageSender<SpawnWorldMesh>,
    pub(crate) static_models: ExternalMessageSender<SpawnStaticModel>,
    pub(crate) mobs: ExternalMessageSender<SpawnMob>,
    pub(crate) movers: ExternalMessageSender<SpawnMover>,
    pub(crate) items: ExternalMessageSender<SpawnItem>,
    pub(crate) objects: ExternalMessageSender<SpawnFromSavedObject>,
}

impl WorldSpawnMessages {
    /// Drops the spawns a world load queued but the pump has not forwarded yet.
    pub(crate) fn clear(&self) {
        self.world_mesh.clear();
        self.static_models.clear();
        self.mobs.clear();
        self.movers.clear();
        self.items.clear();
        self.objects.clear();
    }
}

/// Requests spawning the entities described by a [`SavedObject`]. World load,
/// script inserts and runtime spawns all go through this message.
#[derive(Message, Debug, Clone)]
pub(crate) struct SpawnFromSavedObject {
    pub(crate) object: SavedObject,
}

/// Pre-resolved spawn data of one world vob. Re-spawning it needs only its
/// archive id instead of the full descriptor.
#[derive(Debug, Clone)]
pub(crate) enum VobPrototype {
    Static(SpawnStaticModel),
    Mob(SpawnMob),
    Mover(SpawnMover),
    Item(SpawnItem),
}

/// Spawn prototypes of the currently loaded world, keyed by the vob id in the
/// ZEN archive.
#[derive(Resource, Debug, Default)]
pub(crate) struct VobPrototypes {
    by_id: BTreeMap<u32, VobPrototype>,
    names: HashMap<String, u32>,
}

impl VobPrototypes {
    pub(crate) fn new(by_id: BTreeMap<u32, VobPrototype>, names: HashMap<String, u32>) -> Self {
        Self { by_id, names }
    }

    pub(crate) fn get(&self, vob_id: u32) -> Option<&VobPrototype> {
        self.by_id.get(&vob_id)
    }

    pub(crate) fn id_by_name(&self, name: &str) -> Option<u32> {
        self.names.get(&name.to_lowercase()).copied()
    }

    /// Archive id of the first prototype whose visual model (archetype file
    /// stem) matches `visual`, for the unnamed world vobs.
    pub(crate) fn id_by_visual(&self, visual: &str) -> Option<u32> {
        self.by_id.iter().find_map(|(id, prototype)| {
            let archetype = match prototype {
                VobPrototype::Static(spawn) => &spawn.archetype,
                VobPrototype::Mob(spawn) => &spawn.archetype,
                VobPrototype::Mover(spawn) => &spawn.archetype,
                VobPrototype::Item(_) => return None,
            };
            let stem = std::path::Path::new(archetype).file_stem()?.to_str()?;
            stem.eq_ignore_ascii_case(visual).then_some(*id)
        })
    }

    pub(crate) fn values(&self) -> impl Iterator<Item = &VobPrototype> {
        self.by_id.values()
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
    let _perf = perf_debug!();
    for spawn in spawns.read() {
        let model = std::mem::take(&mut spawn.model);
        commands.spawn((
            Name::new("WorldMesh"),
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
    let _perf = perf_debug!();
    for spawn in spawns.read() {
        if !position_filter.allows(spawn.tr.translation) {
            continue;
        }
        let model_handle = handles_map.get_model_handle(&asset_server, &spawn.archetype, None);
        let name = std::path::Path::new(&spawn.archetype)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("StaticMesh")
            .to_string();
        commands.spawn((
            Name::new(name.clone()),
            ClassifyDistance::default(),
            ZenGinModelComponent {
                model_handle,
                convex_colider: spawn.collides,
                ..default()
            },
            Visibility::default(),
            spawn.tr,
            // Static vobs carry a `MobBase` so their identity is saved by the
            // same payload as every other vob-backed object.
            MobBase {
                vob_id: spawn.vob_id,
                vob_name: name,
                focus_name: String::new(),
            },
            StaticMesh,
        ));
    }
}

/// Turns every queued mob vob into an entity: interactive mobs get `GameMob`,
/// containers get `GameChest` with the loot. Both share `MobBase`, `MobPose` and
/// a selection collider added once the model is loaded.
fn spawn_mobs(
    mut spawns: MessageReader<SpawnMob>,
    mut commands: Commands,
    mut handles_map: ResMut<CachedAssetHandles>,
    asset_server: Res<AssetServer>,
    animation_files: Res<AnimationFiles>,
    position_filter: Res<SpawnPositionFilter>,
) {
    let _perf = perf_debug!();
    let mobs: Vec<SpawnMob> = spawns.read().cloned().collect();
    for mob in mobs {
        if !position_filter.allows(mob.tr.translation) {
            continue;
        }
        let SpawnMob {
            base,
            tr,
            archetype,
            visual_hidden,
            collides,
            scheme,
            visual_stem,
            state_num,
            kind,
        } = mob;
        let no_move = match &kind {
            MobSpawnKind::Interactive(interactive) => interactive.no_move,
            MobSpawnKind::Container(_) => false,
        };
        let model_handle = (!archetype.is_empty())
            .then(|| handles_map.get_model_handle(&asset_server, &archetype, None));
        let name = if base.vob_name.is_empty() {
            base.focus_name.clone()
        } else {
            base.vob_name.clone()
        };
        let mut entity = commands.spawn((
            Name::new(name),
            ClassifyDistance::default(),
            base,
            MobPose {
                scheme,
                visual_stem: visual_stem.clone(),
                state_num,
                positions: None,
                rendered_origin: Vec3::ZERO,
                no_move,
                state: 0,
            },
            Visibility::default(),
            tr,
        ));
        match kind {
            MobSpawnKind::Interactive(interactive) => {
                let InteractiveSpawn {
                    use_with_item,
                    on_state_func,
                    condition_func,
                    rewind,
                    trigger_target,
                    auto_end,
                    ..
                } = interactive;
                entity.insert((
                    GameMob {
                        use_with_item,
                        on_state_func,
                        condition_func,
                        rewind,
                        trigger_target,
                        trigger_fired: false,
                        auto_end,
                    },
                    // Placeholder until the model bounds add the real collider.
                    Collider::sphere(0.5),
                    CollisionLayers::from_bits(SELECTABLE, 0),
                ));
            }
            MobSpawnKind::Container(container) => {
                entity.insert(GameChest {
                    locked: container.locked,
                    key: container.key,
                    pick_string: container.pick_string,
                    cracked: container.cracked,
                    items: container.items,
                });
            }
        }
        if let Some(model_handle) = model_handle {
            // Hidden vobs keep their model only for the `ZS_POS*` attachment
            // nodes: no mesh children, no state animation.
            if visual_hidden {
                entity.insert(MobHiddenModel(model_handle));
                continue;
            }
            let animation_handle = mob_state_animation(
                &animation_files,
                &mut handles_map,
                &asset_server,
                &visual_stem,
                0,
            );
            entity.insert(ZenGinModelComponent {
                model_handle,
                convex_colider: collides,
                animation_handle,
                ..default()
            });
            entity.insert(MobAnimation {
                stem: visual_stem,
                state: 0,
            });
        }
    }
}

/// Publishes named container contents for `Mob_HasItems`. Unnamed chests are
/// skipped, they have no key the scripts could use.
fn sync_script_containers(
    gothic_script: Option<Res<ScriptResource>>,
    chests: Query<(&GameChest, &MobBase)>,
    added: Query<(), Added<GameChest>>,
    changed: Query<(), Changed<GameChest>>,
    mut removed: RemovedComponents<GameChest>,
    mut initialized: Local<bool>,
) {
    let _perf = perf_debug!();
    let dirty =
        !*initialized || !added.is_empty() || !changed.is_empty() || removed.read().count() > 0;
    if !dirty {
        return;
    }
    let Some(gothic_script) = gothic_script else {
        return;
    };
    let containers = chests
        .iter()
        .filter(|(_, base)| !base.vob_name.is_empty())
        .map(|(chest, base)| (base.vob_name.to_lowercase(), chest.items.clone()))
        .collect();
    gothic_script.lock().set_containers(containers);
    *initialized = true;
}

fn spawn_movers(
    mut spawns: MessageReader<SpawnMover>,
    mut commands: Commands,
    mut handles_map: ResMut<CachedAssetHandles>,
    asset_server: Res<AssetServer>,
    position_filter: Res<SpawnPositionFilter>,
) {
    let _perf = perf_debug!();
    for spawn in spawns.read() {
        if !position_filter.allows(spawn.tr.translation) {
            continue;
        }
        let model_handle = (!spawn.archetype.is_empty())
            .then(|| handles_map.get_model_handle(&asset_server, &spawn.archetype, None));
        let mut entity = commands.spawn((
            Name::new(spawn.name.clone()),
            ClassifyDistance::default(),
            MobBase {
                vob_id: spawn.vob_id,
                vob_name: spawn.name.clone(),
                focus_name: String::new(),
            },
            StaticMesh,
            Visibility::default(),
            spawn.tr,
            GameMover::new(
                spawn.name.clone(),
                spawn.keyframes.clone(),
                spawn.tr,
                spawn.speed,
                spawn.start_enabled,
                spawn.behavior,
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

    fn static_model(archetype: &str) -> SpawnStaticModel {
        SpawnStaticModel {
            vob_id: 1,
            tr: Transform::from_xyz(1.0, 2.0, 3.0),
            archetype: archetype.to_string(),
            collides: true,
        }
    }

    #[test]
    fn prototypes_are_keyed_by_vob_id() {
        let prototypes = VobPrototypes::new(
            BTreeMap::from([
                (7, VobPrototype::Static(static_model("a"))),
                (
                    9,
                    VobPrototype::Item(SpawnItem {
                        instance_id: InstanceId::new(1).expect("instance id"),
                        transform: None,
                    }),
                ),
            ]),
            HashMap::new(),
        );

        assert!(matches!(prototypes.get(7), Some(VobPrototype::Static(_))));
        assert!(matches!(prototypes.get(9), Some(VobPrototype::Item(_))));
        assert!(prototypes.get(8).is_none());
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
