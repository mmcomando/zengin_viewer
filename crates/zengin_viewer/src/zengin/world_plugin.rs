use std::f32::consts::PI;

use crate::game_state::AppState;
use crate::gameplay::camera::CameraResource;
use crate::gameplay::character::{
    CharacterCollisions, CharacterController, CharacterMovementSettings, GroundDetection,
    PlayerPhysicsBodyMarker,
};
use crate::gameplay::distance::ClassifyDistance;
use crate::gameplay::game_time::{GameTime, PlayTime};
use crate::gameplay::objects::{NpcModel, PlayerCharacter};
use crate::graphics::NpcAnimations;
use crate::gui::{InventoryUiState, JournalUiState, StatsUiState};
use crate::gui_debug::{NpcVisibility, StaticMesh, WorldMesh};
use crate::save::PendingLoad;
use crate::save::format::SaveGame;
use crate::world_entity::WorldEntityMarker;
use crate::zengin::common::get_full_texture_path;
use crate::zengin::instance_index::InstanceIndex;
use crate::zengin::interaction::selection::Selection;
use crate::zengin::loaders::vfs::{
    item_model_path, mesh_model_path, normalize_mesh_name, resolve_stand_animation,
};
use crate::zengin::npc_routines::{AnimationFiles, WayPoints};
use crate::zengin::objects_to_entities::GameObjectSpawnEntities;
use crate::zengin::resources::{
    CachedAssetHandles, NamedModels, SELECTABLE, ZenGinInsertResources, ZenGinModelComponent,
    ZenGinModelComponentLoaded,
};
use crate::zengin::script::bridge::ScriptChangesApplied;
use crate::zengin::script::component::{GameScriptLoaded, ScriptResource};
use crate::zengin::script::manager::ScriptManager;
use crate::zengin::script::state::{LoadedState, SpawnItem, SpawnNpc};
use crate::zengin::script::string_value::StringCanon;
use crate::zengin::visual::model::ZenGinModel;
use crate::zengin::visual::world::load_zengin_world_data;
use crate::zengin::waypoint_graph::{Spots, WaypointGraph, spawn_waypoint_entities};
use crate::zengin::{
    DialogUiState, GameChest, GameItem, GameNpc, MusicZones, NpcActionQueue, NpcHandle, NpcOverlay,
    ZenginVfs,
};
use avian3d::prelude::*;
use bevy::prelude::*;

/// Loads the zengin world data and spawns the world and player entities.
pub struct ZenGinWorldPlugin;

impl Plugin for ZenGinWorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ZenGinInsertResources);
        app.add_plugins(GameObjectSpawnEntities);

        app.init_resource::<AnimationFiles>();
        app.init_resource::<WayPoints>();
        app.init_resource::<Spots>();
        app.init_resource::<MusicZones>();
        app.init_resource::<WaypointGraph>();
        app.init_resource::<CurrentWorld>();

        app.add_message::<SpawnWorld>();
        app.add_message::<RestartWorld>();
        app.add_message::<NewGame>();
        app.add_systems(
            OnEnter(AppState::LoadScreen),
            request_initial_world.after(GameScriptLoaded),
        );
        app.add_systems(First, restart_world.run_if(on_message::<RestartWorld>));
        app.add_systems(First, new_game.run_if(on_message::<NewGame>));
        app.add_systems(
            Update,
            spawn_world
                .before(ScriptChangesApplied)
                .run_if(in_state(AppState::LoadScreen).or_else(in_state(AppState::InGame))),
        );
        app.add_systems(
            PostUpdate,
            finish_world_load.run_if(in_state(AppState::LoadScreen)),
        );
        app.add_systems(
            Update,
            (spawn_npc_entities, spawn_item_entities)
                .chain()
                .after(ScriptChangesApplied)
                .run_if(in_state(AppState::LoadScreen).or_else(in_state(AppState::InGame))),
        );
    }
}

/// World load request; `world_name` is the path relative to the worlds
/// directory without the `.ZEN` extension (e.g. `NEWWORLD/NEWWORLD`).
#[derive(Message)]
pub struct SpawnWorld {
    pub world_name: String,
    /// World has no saved state, its one-time startup script entry points run.
    pub clean_spawn: bool,
    /// Player position to reuse instead of the default start location.
    pub player_transform: Option<Transform>,
    /// Saved world state to restore. `None` for a normal spawn or a debug
    /// restart.
    pub restore: Option<Box<SaveGame>>,
}

/// Set by the debug UI to tear down and respawn the current world.
#[derive(Message)]
pub struct RestartWorld;

/// Set by the main menu's New Game action to rebuild the script and world state.
#[derive(Message)]
pub struct NewGame;

/// World spawned by a new game.
const START_WORLD: &str = "NEWWORLD/NEWWORLD";

/// World path of the most recently spawned world.
#[derive(Resource, Default)]
pub(crate) struct CurrentWorld(pub(crate) String);

pub(crate) fn request_initial_world(
    mut spawn_world: MessageWriter<SpawnWorld>,
    current_world: Res<CurrentWorld>,
    pending_load: Res<PendingLoad>,
) {
    if pending_load.0.is_some() {
        info!("Skipping the default world spawn, a save load is pending");
        return;
    }
    if !current_world.0.is_empty() {
        info!(
            "World {} already spawned, resuming without reload",
            current_world.0
        );
        return;
    }
    spawn_world.write(SpawnWorld {
        world_name: START_WORLD.to_string(),
        clean_spawn: true,
        player_transform: None,
        restore: None,
    });
}

fn restart_world(world: &mut World) {
    let world_name = world.resource::<CurrentWorld>().0.clone();
    if world_name.is_empty() {
        warn!("World restart requested before a world was spawned");
        return;
    }

    let player_transform = {
        let mut query = world.query_filtered::<&Transform, With<PlayerPhysicsBodyMarker>>();
        query.iter(world).next().copied()
    };
    begin_world_load(world, &world_name, player_transform, None);
}

fn new_game(world: &mut World) {
    begin_world_load(world, START_WORLD, None, None);
}

/// Tears the current world down and requests the next spawn. `restore` carries
/// the save to load; without it the world is rebuilt from scratch.
pub(crate) fn begin_world_load(
    world: &mut World,
    world_name: &str,
    player_transform: Option<Transform>,
    restore: Option<Box<SaveGame>>,
) {
    if *world.resource::<State<AppState>>().get() != AppState::LoadScreen {
        world
            .resource_mut::<NextState<AppState>>()
            .set(AppState::LoadScreen);
    }

    world.resource_mut::<InventoryUiState>().close();
    world.resource_mut::<JournalUiState>().close();
    world.resource_mut::<StatsUiState>().close();
    world.resource_mut::<DialogUiState>().close();
    *world.resource_mut::<Selection>() = Selection::default();

    if let Some(save) = restore.as_deref() {
        world
            .resource_mut::<CameraResource>()
            .set_third_person_camera();
        *world.resource_mut::<GameTime>() = save.metadata.game_time;
        *world.resource_mut::<PlayTime>() = PlayTime {
            seconds: save.metadata.play_time_seconds,
        };
    } else {
        *world.resource_mut::<GameTime>() = GameTime::default();
        *world.resource_mut::<PlayTime>() = PlayTime::default();
    }

    let entities: Vec<Entity> = world
        .query_filtered::<Entity, With<WorldEntityMarker>>()
        .iter(world)
        .collect();
    for entity in entities {
        world.despawn(entity);
    }

    if restore.is_none() {
        let scripts = world.resource::<ScriptResource>().clone();
        let (npc_handles, item_handles) = {
            let mut script = scripts.lock();
            script.restart_world_state();
            (script.npc_handles(), script.item_handles())
        };
        let mut index = world.resource_mut::<InstanceIndex>();
        index.clear();
        index.replace_npc_handles(npc_handles);
        index.replace_item_handles(item_handles);
    } else {
        world.resource_mut::<InstanceIndex>().clear();
    }

    let clean_spawn = restore.is_none();
    world.write_message(SpawnWorld {
        world_name: world_name.to_string(),
        clean_spawn,
        player_transform,
        restore,
    });
}

/// Marks the world ready once the world mesh collider entities exist. Physics
/// and the character controller stay paused until then.
fn finish_world_load(
    world_mesh: Query<&Children, (With<WorldMesh>, With<ZenGinModelComponentLoaded>)>,
    colliders: Query<(), With<Collider>>,
    mut load_state: ResMut<NextState<AppState>>,
) {
    let has_collider = world_mesh
        .iter()
        .any(|children| children.iter().any(|child| colliders.contains(child)));
    if !has_collider {
        return;
    }
    info!("World mesh colliders ready, resuming physics");
    load_state.set(AppState::InGame);
}

fn spawn_world(
    mut requests: MessageReader<SpawnWorld>,
    mut commands: Commands,
    mut handles_map: ResMut<CachedAssetHandles>,
    mut named_models: ResMut<NamedModels>,
    asset_server: Res<AssetServer>,
    vfs: Res<ZenginVfs>,
    gothic_script: Res<ScriptResource>,
    mut instance_index: ResMut<InstanceIndex>,
    mut spawn_item: MessageWriter<SpawnItem>,
    mut spawn_npc: MessageWriter<SpawnNpc>,
) {
    for SpawnWorld {
        world_name,
        clean_spawn,
        player_transform,
        restore,
    } in requests.read()
    {
        spawn_world_data(
            world_name,
            *clean_spawn,
            *player_transform,
            restore.as_deref(),
            &mut commands,
            &mut handles_map,
            &mut named_models,
            &asset_server,
            &vfs,
            &gothic_script,
            &mut instance_index,
            &mut spawn_item,
            &mut spawn_npc,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_world_data(
    world_name: &str,
    clean_spawn: bool,
    player_transform: Option<Transform>,
    restore: Option<&SaveGame>,
    commands: &mut Commands,
    handles_map: &mut CachedAssetHandles,
    named_models: &mut NamedModels,
    asset_server: &Res<AssetServer>,
    vfs: &ZenginVfs,
    gothic_script: &ScriptResource,
    instance_index: &mut InstanceIndex,
    spawn_item: &mut MessageWriter<SpawnItem>,
    spawn_npc: &mut MessageWriter<SpawnNpc>,
) {
    commands.insert_resource(CurrentWorld(world_name.to_string()));
    let _span = info_span!("spawn_world").entered();
    println!("\n-----SCRIPTS VM-----\n");
    let mut gothic_script = gothic_script.lock();
    let loaded_state = gothic_script.loaded_state();
    println!("\n-----LOAD WORLD ZEN DATA-----\n");
    let world_path = format!("/_WORK/DATA/WORLDS/{world_name}.ZEN");
    let mut world_data = {
        let vfs = vfs.0.lock().unwrap();
        load_zengin_world_data(&vfs, &world_path, loaded_state, spawn_item)
    };
    // Keep waypoint positions around for runtime npc routine teleporting,
    // spots take precedence over way points (same lookup order as npc placement).
    let mut way_points = world_data.way_points.clone();
    for (name, tr) in &world_data.spots {
        way_points.insert(name.clone(), *tr);
    }
    // Same table for the script host (`Npc_GetDistToWP`), keys are lowercased
    gothic_script.set_way_points(
        way_points
            .iter()
            .map(|(name, tr)| (name.clone(), tr.translation))
            .collect(),
    );

    println!("\n-----CALL WORLD SCRIPTS-----\n");
    if let Some(save) = restore {
        let (npc_handles, item_handles) =
            gothic_script.restore_world_from_save(world_name, &save.script);
        instance_index.clear();
        instance_index.replace_npc_handles(npc_handles);
        instance_index.replace_item_handles(item_handles);
    } else {
        gothic_script.load_world(world_name, clean_spawn);
    }

    commands.insert_resource(WayPoints { points: way_points });
    commands.insert_resource(Spots(std::mem::take(&mut world_data.spots)));
    commands.insert_resource(AnimationFiles(std::mem::take(&mut world_data.anim_files)));
    commands.insert_resource(MusicZones {
        zones: std::mem::take(&mut world_data.music_zones),
        default: world_data.default_music_zone.take(),
    });
    spawn_waypoint_entities(commands, &world_data.waypoint_graph);
    commands.insert_resource(std::mem::take(&mut world_data.waypoint_graph));

    // Handles of models referenced by name in code, resolved once with the
    // loaded scripts. `None` models are skipped at use time.
    init_named_models(
        named_models,
        gothic_script.loaded_state(),
        vfs,
        asset_server,
        handles_map,
    );

    println!("\n----CREATE ENGINE ENTITIES------\n");

    let world_model_handle: Handle<ZenGinModel> = asset_server.add(world_data.world_model);

    commands.spawn((
        Visibility::default(),
        ClassifyDistance::default(),
        ZenGinModelComponent {
            model_handle: world_model_handle,
            trimesh_collider: true,
            ..default()
        },
        Transform::IDENTITY,
        WorldMesh::default(),
    ));

    for instance in &world_data.static_models {
        let model_handle = handles_map.get_model_handle(asset_server, &instance.archetype, None);
        commands.spawn((
            ClassifyDistance::default(),
            ZenGinModelComponent {
                model_handle: model_handle.clone(),
                convex_colider: instance.collides,
                ..default()
            },
            Visibility::default(),
            instance.tr,
            StaticMesh::default(),
        ));
    }

    for (index, chest) in world_data.chests.into_iter().enumerate() {
        let model_handle = handles_map.get_model_handle(asset_server, &chest.archetype, None);
        let items = restore
            .and_then(|save| save.chests.iter().find(|saved| saved.index == index))
            .map_or(chest.items, |saved| saved.items.clone());
        commands.spawn((
            ClassifyDistance::default(),
            GameChest {
                index,
                name: chest.name,
                locked: chest.locked,
                items,
            },
            Collider::cuboid(
                chest.collider_half_extents.x * 2.0,
                chest.collider_half_extents.y * 2.0,
                chest.collider_half_extents.z * 2.0,
            ),
            CollisionLayers::from_bits(SELECTABLE, 0),
            Visibility::default(),
            chest.tr,
            ZenGinModelComponent {
                model_handle,
                convex_colider: chest.collides,
                ..default()
            },
            StaticMesh::default(),
        ));
    }

    if !gothic_script.has_player_instance() {
        spawn_player(
            commands,
            &mut gothic_script,
            instance_index,
            player_transform,
        );
    }

    if let Some(save) = restore {
        for npc in &save.live_npcs {
            spawn_npc.write(SpawnNpc {
                instance_id: npc.instance_id,
                location_name: None,
                transform: Some(npc.transform),
                exact_transform: true,
                overlay: npc.overlay.clone(),
            });
        }
        for item in &save.live_items {
            spawn_item.write(SpawnItem {
                instance_id: item.instance_id,
                location_name: None,
                transform: Some(item.transform),
            });
        }
    }
}

/// Resolves the handles of models referenced by name in code, called at world
/// load after the scripts are loaded. `None` models are skipped at use time.
fn init_named_models(
    named_models: &mut NamedModels,
    loaded_state: &LoadedState,
    vfs: &ZenginVfs,
    asset_server: &Res<AssetServer>,
    handles: &mut CachedAssetHandles,
) {
    named_models.item_arrow = loaded_state
        .items_instances
        .iter()
        .find(|(_, instance)| {
            instance
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .name
                .eq_ignore_ascii_case("ITRW_ARROW")
        })
        .and_then(|(_, instance)| {
            let instance = instance
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            mesh_model_path(vfs, instance.chosen_visual())
        })
        .map(|path| handles.get_model_handle(asset_server, &path, None));
    if named_models.item_arrow.is_none() {
        warn!("No ITRW_ARROW item instance found, arrows will have no visual");
    }
}

/// Spawns the player entity: a physics collider with the `PC_HERO` visual as a
/// child. The player is the `PC_HERO` instance, the `hero` instance global has
/// no data of its own (its stats come from `PC_HERO`).
fn spawn_player(
    commands: &mut Commands,
    gothic_script: &mut ScriptManager,
    instance_index: &mut InstanceIndex,
    player_transform: Option<Transform>,
) {
    let player_instance = gothic_script
        .symbol_instance("pc_hero")
        .expect("player instance pc_hero should be a valid instance symbol");
    gothic_script.set_player_instance(player_instance);
    let Some(handle) = gothic_script.npc_handle(player_instance) else {
        warn!(
            "No npc handle for player instance({}), player visual skipped",
            player_instance.value()
        );
        return;
    };
    instance_index.insert_npc_handle(handle.clone());
    let mut player_collider = commands.spawn((
        Visibility::default(),
        PlayerPhysicsBodyMarker,
        CharacterController,
        CharacterMovementSettings::default(),
        CharacterCollisions::default(),
        GroundDetection {
            // Use a slightly smaller capsule for shape casts used for ground detection
            cast_shape: Some(Collider::capsule(0.499, 0.8)),
            max_angle: PI / 3.0,
            max_distance: 0.2,
        },
        Collider::capsule(0.4, 1.2),
        TransformInterpolation,
        // Mesh3d(meshes.add(Capsule3d::new(0.5, 0.8))),
        // MeshMaterial3d(materials.add(Color::srgb(0.8, 0.7, 0.6))),
        // Transform::from_xyz(0.0, 1.5, 0.0),
        player_transform.unwrap_or(Transform::from_xyz(-97.0, 6.0, -7.0)),
    ));

    // Player visual
    player_collider.with_child((
        Visibility::default(),
        Transform::from_xyz(0.0, -1.0, 0.0),
        PlayerCharacter,
        GameNpc::new(handle),
        NpcModel {
            hierarchy: Some("zengin://_WORK/DATA/ANIMS/_COMPILED/HUMANS.MDH".to_string()),
            body_model: "zengin://_WORK/DATA/ANIMS/_COMPILED/HUM_BODY_NAKED0.MDM".to_string(),
            body_texture: Some(
                "zengin://_WORK/DATA/TEXTURES/_COMPILED/HUM_BODY_NAKED_V9_C0-C.TEX".to_string(),
            ),
            head_model: Some("zengin://_WORK/DATA/ANIMS/_COMPILED/HUM_HEAD_PONY.MMB".to_string()),
            head_texture: Some(
                "zengin://_WORK/DATA/TEXTURES/_COMPILED/HUM_HEAD_V18_C0-C.TEX".to_string(),
            ),
            // armor_model: None,
            // armor_model: Some("zengin://_WORK/DATA/ANIMS/_COMPILED/ARMOR_SLD_H.MDL".to_string()),
            // armor_model: Some("zengin://_WORK/DATA/ANIMS/_COMPILED/ARMOR_PAL_H.MDM".to_string()),
            stand_animation: Some("S_RUN".to_string()),
            animation_scheme: Some("HUMANS".to_string()),
            model_fatness: None,
        },
        NpcAnimations { animation: None },
    ));
}

/// Data needed to spawn one npc entity, extracted from the script state in a
/// single lock.
struct NpcSpawnScript {
    handle: NpcHandle,
    body_model: String,
    body_texture: Option<String>,
    head_model: Option<String>,
    face_texture: Option<String>,
    hierarchy: Option<String>,
    model_scale: Option<Vec3>,
    model_fatness: Option<f32>,
    /// Waypoint of the routine entry active at spawn time, takes precedence
    /// over the spawn location.
    routine_way_point: Option<StringCanon>,
    location_name: Option<StringCanon>,
}

fn npc_spawn_script_data(
    scripts: &ScriptResource,
    spawn: &SpawnNpc,
    minutes_of_day: u32,
) -> Option<NpcSpawnScript> {
    let mut script = scripts.lock();
    // Resolve the location string first, the instance state borrow below is
    // immutable while `get_canon_string` needs `&mut`
    let location_name = spawn
        .location_name
        .and_then(|id| script.get_canon_string(id));
    let handle = script.npc_handle(spawn.instance_id)?;
    let state = script.get_loaded_state();
    let instance = state.npc_instances.get(&spawn.instance_id)?;
    let instance = instance
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let routine_way_point = instance
        .get_routine_entry(minutes_of_day)
        .map(|entry| entry.way_point.clone());
    Some(NpcSpawnScript {
        handle,
        body_model: instance.visual.body_model.clone(),
        body_texture: instance.visual.body_texture.clone(),
        head_model: instance.visual.head_model.clone(),
        face_texture: instance.visual.face_texture.clone(),
        hierarchy: instance.visual.hierarchy.clone(),
        model_scale: instance.visual.model_scale,
        model_fatness: instance.visual.model_fatness,
        routine_way_point,
        location_name,
    })
}

/// Spawns a full npc entity for every requested npc, resolving its placement
/// from the routine/spawn location and its visuals from the script state and
/// the shared vfs.
#[allow(clippy::cognitive_complexity)]
fn spawn_npc_entities(
    mut spawns: MessageReader<SpawnNpc>,
    mut commands: Commands,
    gothic_scripts: Res<ScriptResource>,
    vfs: Res<ZenginVfs>,
    way_points: Res<WayPoints>,
    spots: Res<Spots>,
    game_time: Res<GameTime>,
    mut instance_index: ResMut<InstanceIndex>,
) {
    for spawn in spawns.read() {
        let Some(script) = npc_spawn_script_data(&gothic_scripts, spawn, game_time.hour() * 60)
        else {
            warn!("No instance state for npc({}) spawn", spawn.instance_id);
            continue;
        };
        instance_index.insert_npc_handle(script.handle.clone());

        let mut transform = if let Some(tr) = spawn.transform {
            tr
        } else {
            // Routine waypoint takes precedence over the spawn location,
            // spots take precedence over way points
            let name = script
                .routine_way_point
                .or(script.location_name)
                .unwrap_or_default();
            let tr = way_points
                .points
                .get(name.as_str())
                .or_else(|| spots.0.get(name.as_str()));
            let Some(tr) = tr else {
                warn!(
                    "No location found for npc({}) on({})",
                    spawn.instance_id, name
                );
                continue;
            };
            *tr
        };
        // Place the visual on the ground, matching the engine placement. A
        // restore spawn uses the saved transform verbatim.
        if !spawn.exact_transform {
            transform.translation.y -= 0.5;
            if let Some(scale) = script.model_scale {
                transform.scale = scale;
            }
        }

        let Some(body_model) = mesh_model_path(&vfs, &script.body_model) else {
            warn!(
                "No mesh for npc({}) body model({})",
                spawn.instance_id, script.body_model
            );
            continue;
        };
        let head_model = script.head_model.as_deref().and_then(|name| {
            let path = mesh_model_path(&vfs, name);
            if path.is_none() {
                warn!(
                    "No mesh for npc({}) head model({})",
                    spawn.instance_id, name
                );
            }
            path
        });
        let hierarchy = script.hierarchy.as_deref().and_then(|name| {
            let path = mesh_model_path(&vfs, name);
            if path.is_none() {
                warn!("No mesh for npc({}) hierarchy({})", spawn.instance_id, name);
            }
            path
        });

        // Animations of an npc live in the scheme of its visual (`.mds`), the
        // body mesh name is unrelated, e.g. the wolf visual `Wolf.mds` uses
        // `WOLF-*.MAN` while its body mesh is `Wol_Body`.
        let animation_scheme = script.hierarchy.as_deref().map(normalize_mesh_name);
        let stand_animation = animation_scheme
            .as_deref()
            .and_then(|scheme| resolve_stand_animation(&vfs, scheme));

        let mut entity = commands.spawn((
            Visibility::default(),
            GameNpc::new(script.handle.clone()),
            NpcActionQueue::default(),
            NpcModel {
                hierarchy,
                body_model,
                body_texture: script.body_texture.as_deref().map(get_full_texture_path),
                head_model,
                head_texture: script.face_texture.as_deref().map(get_full_texture_path),
                animation_scheme,
                stand_animation,
                model_fatness: script.model_fatness,
            },
            NpcAnimations::default(),
            transform,
            NpcVisibility::default(),
        ));
        if let Some(overlay) = &spawn.overlay {
            entity.insert(NpcOverlay(Some(overlay.clone())));
        }
    }
}

/// Spawns an item entity, resolving its model from the script item instance.
#[allow(clippy::cognitive_complexity)]
fn spawn_item_entities(
    mut spawns: MessageReader<SpawnItem>,
    mut commands: Commands,
    mut handles_map: ResMut<CachedAssetHandles>,
    asset_server: Res<AssetServer>,
    gothic_scripts: ResMut<ScriptResource>,
    way_points: Res<WayPoints>,
    spots: Res<Spots>,
    vfs: Res<ZenginVfs>,
    mut instance_index: ResMut<InstanceIndex>,
) {
    for spawn in spawns.read() {
        let Some(handle) = gothic_scripts.lock().item_handle(spawn.instance_id) else {
            warn!(
                "No item handle for item({}) spawn",
                spawn.instance_id.value()
            );
            continue;
        };
        instance_index.insert_item_handle(handle.clone());
        let Some(model) = item_model_path(&gothic_scripts, &vfs, spawn.instance_id) else {
            crate::warn_once!("No mesh path for item({})", spawn.instance_id);
            continue;
        };
        let waypoint_tr = if let Some(tr) = spawn.transform {
            tr
        } else if let Some(waypoint_id) = spawn.location_name {
            let Some(location_name) = gothic_scripts.lock().get_canon_string(waypoint_id) else {
                warn!("Empty location for item({})", spawn.instance_id);
                continue;
            };
            let waypoint_tr =
                if let Some(waypoint_tr) = way_points.points.get(location_name.as_str()) {
                    *waypoint_tr
                } else if let Some(waypoint_tr) = spots.0.get(location_name.as_str()) {
                    *waypoint_tr
                } else {
                    warn!(
                        "No location found for item({}) on({})",
                        spawn.instance_id, location_name
                    );
                    continue;
                };

            // let Some(waypoint_tr) = way_points.points.get(&waypoint_name) else {
            //     warn!(
            //         "No location found for item({}) on({})",
            //         spawn.instance_id, waypoint_name
            //     );
            //     continue;
            // };
            waypoint_tr
        } else {
            continue;
        };

        let model_handle = handles_map.get_model_handle(&asset_server, &model, None);

        commands.spawn((
            ClassifyDistance::default(),
            ZenGinModelComponent {
                model_handle,
                ..default()
            },
            Visibility::default(),
            waypoint_tr,
            GameItem::new(handle),
        ));
    }
}
