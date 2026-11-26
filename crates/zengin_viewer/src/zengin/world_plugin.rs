use std::collections::VecDeque;
use std::f32::consts::PI;

use crate::game_state::AppState;
use crate::gameplay::camera::CameraResource;
use crate::gameplay::character::{
    CharacterCollisions, CharacterController, CharacterMovementSettings, GroundDetection,
    PLAYER_MODEL_ROOT_TO_CAPSULE_CENTER,
};
use crate::gameplay::distance::ClassifyDistance;
use crate::gameplay::game_time::{GameTime, PlayTime};
use crate::gameplay::objects::{NpcModel, PlayerCharacter};
use crate::graphics::{NpcAnimations, PlayerMeleeCombo};
use crate::gui::{InventoryUiState, JournalUiState, StatsUiState};
use crate::gui_debug::WorldMesh;
use crate::input::Options;
use crate::perf_debug;
use crate::save::PendingLoad;
use crate::save::format::{SaveGame, SavedChest, SavedNpc, SavedObject, SavedObjectBuilder};
use crate::world_entity::WorldEntityMarker;
use crate::zengin::change_level::ChangeLevelTriggers;
use crate::zengin::common::get_full_texture_path;
use crate::zengin::instance_index::InstanceIndex;
use crate::zengin::interaction::selection::Selection;
use crate::zengin::loaders::vfs::{
    item_model_path, mesh_body_path, mesh_model_path, normalize_mesh_name, resolve_stand_animation,
};
use crate::zengin::movers::PendingWorldStartTriggers;
use crate::zengin::npc_routines::WayPoints;
use crate::zengin::objects_to_entities::GameObjectSpawnEntities;
use crate::zengin::resources::{
    CachedAssetHandles, NamedModels, ZenGinInsertResources, ZenGinModelComponent,
    ZenGinModelComponentLoaded,
};
use crate::zengin::script::bridge::ScriptChangesApplied;
use crate::zengin::script::component::{GameScriptLoaded, ScriptResource};
use crate::zengin::script::manager::ScriptManager;
use crate::zengin::script::state::{LoadedState, RoutineEntry, SpawnItem, SpawnNpc};
use crate::zengin::script::string_value::StringCanon;
use crate::zengin::visual::world::load_zengin_world_data;
use crate::zengin::waypoint_graph::{Spots, WaypointGraph, spawn_waypoint_entities};
use crate::zengin::world_spawn::{
    MobSpawnKind, SpawnFromSavedObject, SpawnPositionFilter, VobPrototype, VobPrototypes,
    WorldSpawnMessages, WorldSpawnPlugin,
};
use crate::zengin::{
    DialogUiState, GameItem, GameNpc, MusicZones, NpcActionQueue, NpcHandle, NpcOverlay, ZenginVfs,
    talent_overlay_for,
};
use avian3d::prelude::*;
use bevy::prelude::*;

/// Loads the zengin world data and spawns the world and player entities.
pub struct ZenGinWorldPlugin;

impl Plugin for ZenGinWorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ZenGinInsertResources);
        app.add_plugins(WorldSpawnPlugin);
        app.add_plugins(GameObjectSpawnEntities);

        app.init_resource::<WayPoints>();
        app.init_resource::<Spots>();
        app.init_resource::<MusicZones>();
        app.init_resource::<WaypointGraph>();
        app.init_resource::<CurrentWorld>();
        app.init_resource::<LoadingWorld>();
        app.init_resource::<PendingWorldSpawns>();
        app.init_resource::<NpcSpawnNameFilter>();
        app.init_resource::<LoadScreenElapsed>();

        app.add_message::<RestartWorld>();
        app.add_message::<NewGame>();
        app.add_message::<SpawnPlayer>();
        app.add_message::<SpawnNpc>();
        app.add_systems(
            OnEnter(AppState::LoadScreen),
            (reset_load_screen_elapsed, request_initial_world),
        );
        app.add_systems(First, restart_world.run_if(on_message::<RestartWorld>));
        app.add_systems(First, new_game.run_if(on_message::<NewGame>));
        app.add_systems(
            Update,
            spawn_player_entities
                .after(GameScriptLoaded)
                .after(spawn_world)
                .before(ScriptChangesApplied)
                .run_if(resource_exists::<ScriptResource>)
                .run_if(in_state(AppState::LoadScreen).or_else(in_state(AppState::InGame))),
        );
        app.add_systems(
            Update,
            spawn_world
                .after(GameScriptLoaded)
                .before(ScriptChangesApplied)
                .run_if(crate::game_state::loading_screen_ready)
                .run_if(resource_exists::<ScriptResource>)
                .run_if(in_state(AppState::LoadScreen).or_else(in_state(AppState::InGame))),
        );
        app.add_systems(
            PostUpdate,
            finish_world_load
                .run_if(in_state(AppState::LoadScreen))
                // In-game systems assume the game script exists.
                .run_if(resource_exists::<ScriptResource>),
        );
        app.add_systems(
            Update,
            (spawn_npc_entities, spawn_item_entities)
                .chain()
                .after(ScriptChangesApplied)
                .run_if(resource_exists::<ScriptResource>)
                .run_if(in_state(AppState::LoadScreen).or_else(in_state(AppState::InGame))),
        );
        app.add_systems(
            Update,
            spawn_from_saved_objects
                .after(ScriptChangesApplied)
                .run_if(resource_exists::<ScriptResource>)
                .run_if(resource_exists::<VobPrototypes>)
                .run_if(in_state(AppState::LoadScreen).or_else(in_state(AppState::InGame))),
        );
    }
}

/// World load request; `world_name` is the path relative to the worlds
/// directory without the `.ZEN` extension (e.g. `NEWWORLD/NEWWORLD`).
pub struct SpawnWorld {
    pub world_name: String,
    /// World has no saved state, its one-time startup script entry points run.
    pub clean_spawn: bool,
    /// Player position to reuse instead of the default start location.
    pub player_transform: Option<Transform>,
    /// Vob name in `world_name` the player is placed at (level change).
    pub start_vob: Option<String>,
    /// Saved world state to restore. `None` for a normal spawn or a debug
    /// restart.
    pub restore: Option<Box<SaveGame>>,
}

/// World spawn requests waiting for the loading screen; drained by
/// [`spawn_world`].
#[derive(Resource, Default)]
pub(crate) struct PendingWorldSpawns(VecDeque<SpawnWorld>);

/// Player character spawn request; `transform` overrides the default start
/// location.
#[derive(Message)]
struct SpawnPlayer {
    transform: Option<Transform>,
}

/// Set by the debug UI to tear down and respawn the current world.
#[derive(Message)]
pub struct RestartWorld;

/// Set by the main menu's New Game action to rebuild the script and world state.
#[derive(Message)]
pub struct NewGame;

/// World spawned by a new game.
pub(crate) const START_WORLD: &str = "NEWWORLD/NEWWORLD";

/// World path of the most recently spawned world.
#[derive(Resource, Default)]
pub(crate) struct CurrentWorld(pub(crate) String);

/// World path of the world being spawned or restored, used to pick the
/// loading screen artwork before the world exists.
#[derive(Resource)]
pub(crate) struct LoadingWorld(pub(crate) String);

impl Default for LoadingWorld {
    fn default() -> Self {
        Self(START_WORLD.to_string())
    }
}

/// Total time spent in [`AppState::LoadScreen`].
#[derive(Resource, Default)]
struct LoadScreenElapsed(f32);

/// Case-insensitive substring filters matched against an npc's script symbol
/// and display names. An empty list matches every npc.
#[derive(Resource, Default)]
pub(crate) struct NpcSpawnNameFilter(Vec<String>);

impl NpcSpawnNameFilter {
    pub(crate) fn add(&mut self, filters: &[String]) {
        self.0
            .extend(filters.iter().map(|filter| filter.to_lowercase()));
    }

    fn allows(&self, symbol_name: &str, display_name: Option<&str>) -> bool {
        if self.0.is_empty() {
            return true;
        }
        let symbol_name = symbol_name.to_lowercase();
        let display_name = display_name.map(str::to_lowercase);
        self.0.iter().any(|filter| {
            symbol_name.contains(filter)
                || display_name
                    .as_ref()
                    .is_some_and(|name| name.contains(filter))
        })
    }
}

pub(crate) fn request_initial_world(
    mut pending_spawns: ResMut<PendingWorldSpawns>,
    current_world: Res<CurrentWorld>,
    pending_load: Res<PendingLoad>,
    mut loading_world: ResMut<LoadingWorld>,
    options: Res<Options>,
) {
    let _perf = perf_debug!();
    if options.scenario_enabled() {
        info!("Skipping the default world spawn, the scenario run requests it explicitly");
        return;
    }
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
    *loading_world = LoadingWorld(START_WORLD.to_string());
    pending_spawns.0.push_back(clean_spawn_request(START_WORLD));
}

/// Queues a spawn of `world_name` without touching script state, so it can
/// run before the game script is loaded.
pub(crate) fn request_world_spawn(world: &mut World, world_name: &str) {
    let spawn = clean_spawn_request(world_name);
    world
        .resource_mut::<PendingWorldSpawns>()
        .0
        .push_back(spawn);
    *world.resource_mut::<LoadingWorld>() = LoadingWorld(world_name.to_string());
}

/// Requests a player character spawn.
pub(crate) fn request_player_spawn(world: &mut World, transform: Transform) {
    let Some(spawns) = world.get_resource::<WorldSpawnMessages>() else {
        warn!("Player spawn requested before the spawn message is registered");
        return;
    };
    spawns.objects.send(SpawnFromSavedObject {
        object: SavedObjectBuilder::new(transform).player().build(),
    });
}

/// A request for a fresh world without saved state.
fn clean_spawn_request(world_name: &str) -> SpawnWorld {
    SpawnWorld {
        world_name: world_name.to_string(),
        clean_spawn: true,
        player_transform: None,
        start_vob: None,
        restore: None,
    }
}

fn restart_world(world: &mut World) {
    let _perf = perf_debug!();
    let world_name = world.resource::<CurrentWorld>().0.clone();
    if world_name.is_empty() {
        warn!("World restart requested before a world was spawned");
        return;
    }

    let player_transform = {
        let mut query = world.query_filtered::<&Transform, With<PlayerCharacter>>();
        query.iter(world).next().copied()
    };
    begin_world_load(world, &world_name, player_transform, None);
}

fn new_game(world: &mut World) {
    let _perf = perf_debug!();
    begin_world_load(world, START_WORLD, None, None);
}

/// Spawns the player character for every request, skipping duplicates.
fn spawn_player_entities(
    mut requests: MessageReader<SpawnPlayer>,
    mut commands: Commands,
    scripts: Res<ScriptResource>,
    mut instance_index: ResMut<InstanceIndex>,
    players: Query<(), With<PlayerCharacter>>,
) {
    let _perf = perf_debug!();
    let mut player_exists = players.iter().next().is_some();
    for request in requests.read() {
        if player_exists {
            warn!("Player character spawn requested, but the player already exists");
            continue;
        }
        let mut script = scripts.lock();
        spawn_player(
            &mut commands,
            &mut script,
            &mut instance_index,
            request.transform,
        );
        player_exists = true;
        info!("Spawned the player character on request");
    }
}

/// Tears the current world down and requests the next spawn. `restore` carries
/// the save to load; without it the world is rebuilt from scratch.
pub(crate) fn begin_world_load(
    world: &mut World,
    world_name: &str,
    player_transform: Option<Transform>,
    restore: Option<Box<SaveGame>>,
) {
    begin_world_load_with(world, world_name, player_transform, None, restore, true);
}

/// Loads another world without resetting the script globals, used by
/// `oCTriggerChangeLevel`. The player is placed at `start_vob` in the new world.
pub(crate) fn begin_level_change(world: &mut World, world_name: &str, start_vob: &str) {
    info!(
        world = %world_name,
        start_vob,
        "Loading world after a level change"
    );
    begin_world_load_with(
        world,
        world_name,
        None,
        Some(start_vob.to_string()),
        None,
        false,
    );
}

/// `reset_script` distinguishes a fresh world (new game, debug restart) from a
/// level change, which keeps the script globals and the player instance.
fn begin_world_load_with(
    world: &mut World,
    world_name: &str,
    player_transform: Option<Transform>,
    start_vob: Option<String>,
    restore: Option<Box<SaveGame>>,
    reset_script: bool,
) {
    if *world.resource::<State<AppState>>().get() != AppState::LoadScreen {
        world
            .resource_mut::<NextState<AppState>>()
            .set(AppState::LoadScreen);
    }

    world.resource::<WorldSpawnMessages>().clear();
    // Spawn messages written straight into the ECS buffers skip the external
    // inbox clear above; do not let them leak into the next world.
    if let Some(mut spawns) = world.get_resource_mut::<Messages<SpawnItem>>() {
        spawns.clear();
    }
    if let Some(mut spawns) = world.get_resource_mut::<Messages<SpawnPlayer>>() {
        spawns.clear();
    }
    if let Some(mut spawns) = world.get_resource_mut::<Messages<SpawnFromSavedObject>>() {
        spawns.clear();
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
    } else if reset_script {
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
        if reset_script {
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
            // Script globals and the player instance survive a level change;
            // the handles of the new world are re-registered as it spawns.
            world.resource_mut::<InstanceIndex>().clear();
        }
    } else {
        world.resource_mut::<InstanceIndex>().clear();
    }

    let clean_spawn = restore.is_none();
    *world.resource_mut::<LoadingWorld>() = LoadingWorld(world_name.to_string());
    world
        .resource_mut::<PendingWorldSpawns>()
        .0
        .push_back(SpawnWorld {
            world_name: world_name.to_string(),
            clean_spawn,
            player_transform,
            start_vob,
            restore,
        });
}

/// Fallback for a world mesh that never produces a collider, so a broken asset
/// cannot trap the app on the loading screen.
const LOAD_TIMEOUT: f32 = 100.0;

fn reset_load_screen_elapsed(mut elapsed: ResMut<LoadScreenElapsed>) {
    let _perf = perf_debug!();
    elapsed.0 = 0.0;
}

/// Marks the world ready once the world mesh collider entities exist. Physics
/// and the character controller stay paused until then, but [`LOAD_TIMEOUT`]
/// resumes them anyway so a missing collider does not block the game forever.
fn finish_world_load(
    time: Res<Time>,
    mut elapsed: ResMut<LoadScreenElapsed>,
    world_mesh: Query<&Children, (With<WorldMesh>, With<ZenGinModelComponentLoaded>)>,
    colliders: Query<(), With<Collider>>,
    mut load_state: ResMut<NextState<AppState>>,
) {
    let _perf = perf_debug!();
    elapsed.0 += time.delta_secs();
    let has_collider = world_mesh
        .iter()
        .any(|children| children.iter().any(|child| colliders.contains(child)));
    if has_collider {
        info!("World mesh colliders ready, resuming physics");
    } else if elapsed.0 >= LOAD_TIMEOUT {
        warn!(
            "World mesh colliders still missing after {LOAD_TIMEOUT}s, resuming physics without them"
        );
    } else {
        return;
    }
    load_state.set(AppState::InGame);
}

fn spawn_world(
    mut pending_spawns: ResMut<PendingWorldSpawns>,
    mut commands: Commands,
    mut handles_map: ResMut<CachedAssetHandles>,
    mut named_models: ResMut<NamedModels>,
    asset_server: Res<AssetServer>,
    vfs: Res<ZenginVfs>,
    gothic_script: Res<ScriptResource>,
    mut instance_index: ResMut<InstanceIndex>,
    spawns: Res<WorldSpawnMessages>,
    options: Res<Options>,
) {
    let _perf = perf_debug!();
    if pending_spawns.0.is_empty() {
        return;
    }
    // Scenario runs spawn the player character explicitly, an interactive
    // run gets it with the world.
    let spawn_default_player = !options.scenario_enabled();
    for SpawnWorld {
        world_name,
        clean_spawn,
        player_transform,
        start_vob,
        restore,
    } in std::mem::take(&mut pending_spawns.0)
    {
        spawn_world_data(
            &world_name,
            clean_spawn,
            player_transform,
            start_vob.as_deref(),
            restore.as_deref(),
            spawn_default_player,
            &mut commands,
            &mut handles_map,
            &mut named_models,
            &asset_server,
            &vfs,
            &gothic_script,
            &mut instance_index,
            &spawns,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_world_data(
    world_name: &str,
    clean_spawn: bool,
    player_transform: Option<Transform>,
    start_vob: Option<&str>,
    restore: Option<&SaveGame>,
    spawn_default_player: bool,
    commands: &mut Commands,
    handles_map: &mut CachedAssetHandles,
    named_models: &mut NamedModels,
    asset_server: &Res<AssetServer>,
    vfs: &ZenginVfs,
    gothic_script: &ScriptResource,
    instance_index: &mut InstanceIndex,
    spawns: &WorldSpawnMessages,
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
        load_zengin_world_data(&vfs, &world_path, loaded_state, spawns, true)
    };
    // Every spawnable vob was resolved while the ZEN was parsed. Loaded state
    // replaces the world-definition data of the same vob, then the initial
    // world is spawned from the prototypes, which stay available for later
    // respawns.
    let prototypes = VobPrototypes::new(
        std::mem::take(&mut world_data.prototypes),
        std::mem::take(&mut world_data.names),
    );
    // `zCTriggerWorldStart` events are fired once the world is in game, after
    // the movers they address exist. A restored world only fires the triggers
    // that are not limited to the first start.
    let world_start_triggers = world_data
        .world_start_triggers
        .drain(..)
        .filter(|trigger| clean_spawn || !trigger.fire_only_first_time)
        .map(|trigger| trigger.target)
        .collect();
    commands.insert_resource(PendingWorldStartTriggers(world_start_triggers));
    commands.insert_resource(ChangeLevelTriggers(std::mem::take(
        &mut world_data.change_level_triggers,
    )));
    // Keep waypoint positions around for runtime npc routine teleporting,
    // spots take precedence over way points (same lookup order as npc placement).
    let mut way_points = world_data.way_points.clone();
    for (name, tr) in &world_data.spots {
        way_points.insert(name.clone(), *tr);
    }
    // A level change names the vob the player appears at instead of a full
    // transform; spots and way points are keyed by their lowercase name.
    let force_player_spawn = start_vob.is_some();
    let player_transform = start_vob
        .and_then(|name| {
            let tr = way_points.get(&name.to_lowercase()).copied();
            if tr.is_none() {
                warn!("Start vob({name}) of world({world_name}) not found");
            }
            tr
        })
        .or(player_transform);
    if let (true, Some(tr)) = (force_player_spawn, player_transform) {
        info!(
            start_vob = start_vob.unwrap_or_default(),
            position = ?tr.translation,
            "Placing the player at the level start vob"
        );
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

    // Scenarios spawn the player character explicitly with
    // `ZenginSpawnPlayerCharacter`. A clean spawn gets its player here, a
    // restored world gets the player object from the save.
    let spawn_player_entity = (spawn_default_player || player_transform.is_some())
        && (force_player_spawn || !gothic_script.has_player_instance());

    if let Some(save) = restore {
        for object in &save.objects {
            if object.player.is_some() && !spawn_player_entity {
                continue;
            }
            spawns.objects.send(SpawnFromSavedObject {
                object: object.clone(),
            });
        }
    } else {
        for object in prototypes.values().filter_map(saved_object_from_prototype) {
            spawns.objects.send(SpawnFromSavedObject { object });
        }
        if spawn_player_entity {
            spawns.objects.send(SpawnFromSavedObject {
                object: SavedObjectBuilder::new(
                    player_transform.unwrap_or_else(default_player_transform),
                )
                .player()
                .build(),
            });
        }
    }
    commands.insert_resource(prototypes);
}

/// Loads only the spawn prototypes of `world`, without spawning its entities or
/// touching the scenario waypoints/spots.
pub(crate) fn load_world_prototypes(world: &mut World, world_name: &str) {
    let world_path = format!("/_WORK/DATA/WORLDS/{world_name}.ZEN");
    let data = {
        let (Some(vfs), Some(scripts), Some(spawns)) = (
            world.get_resource::<ZenginVfs>(),
            world.get_resource::<ScriptResource>(),
            world.get_resource::<WorldSpawnMessages>(),
        ) else {
            warn!("World prototype load requested before the game script or vfs is ready");
            return;
        };
        let mut scripts = scripts.lock();
        let vfs = vfs.0.lock().unwrap();
        load_zengin_world_data(&vfs, &world_path, scripts.loaded_state(), spawns, false)
    };
    world.insert_resource(VobPrototypes::new(data.prototypes, data.names));
    info!("Loaded the world prototypes of {world_name}");
}

/// Turns a world prototype into the spawn descriptor a fresh world sends to
/// [`spawn_from_saved_objects`].
fn saved_object_from_prototype(prototype: &VobPrototype) -> Option<SavedObject> {
    let object = match prototype {
        VobPrototype::Static(spawn) => SavedObjectBuilder::new(spawn.tr)
            .mob_base(spawn.vob_id)
            .static_object()
            .build(),
        VobPrototype::Mover(spawn) => SavedObjectBuilder::new(spawn.tr)
            .mob_base(spawn.vob_id)
            .static_object()
            .build(),
        VobPrototype::Mob(spawn) => {
            let mut builder = SavedObjectBuilder::new(spawn.tr).mob_base(spawn.base.vob_id);
            if let MobSpawnKind::Container(container) = &spawn.kind {
                builder = builder.chest(SavedChest {
                    locked: container.locked,
                    items: container.items.clone(),
                    cracked: container.cracked,
                });
            }
            builder.build()
        }
        VobPrototype::Item(spawn) => {
            let Some(transform) = spawn.transform else {
                warn!(
                    "Item prototype item({}) has no transform",
                    spawn.instance_id.value()
                );
                return None;
            };
            SavedObjectBuilder::new(transform)
                .item(spawn.instance_id)
                .build()
        }
    };
    Some(object)
}

/// Start position of the player when a world spawn has no explicit one.
fn default_player_transform() -> Transform {
    Transform::from_xyz(-97.0, 5.0, -7.0)
}

fn spawn_from_saved_objects(
    mut requests: MessageReader<SpawnFromSavedObject>,
    way_points: Res<WayPoints>,
    spots: Res<Spots>,
    prototypes: Res<VobPrototypes>,
    spawns: Res<WorldSpawnMessages>,
    mut spawn_npc: MessageWriter<SpawnNpc>,
    mut player_spawn: MessageWriter<SpawnPlayer>,
) {
    let _perf = perf_debug!();
    for request in requests.read() {
        let object = &request.object;
        if object.player.is_some() {
            let Some(transform) = saved_object_transform(object, &way_points, &spots) else {
                continue;
            };
            player_spawn.write(SpawnPlayer {
                transform: Some(transform),
            });
            continue;
        }
        if let Some(npc) = &object.npc {
            spawn_npc.write(npc_spawn_request(npc, object));
        }
        if object.item.is_none() && object.mob_base.is_none() {
            continue;
        }
        let Some(transform) = saved_object_transform(object, &way_points, &spots) else {
            continue;
        };
        if let Some(item) = &object.item {
            spawns.items.send(SpawnItem {
                instance_id: item.instance_id,
                transform: Some(transform),
            });
        }
        let Some(mob_base) = &object.mob_base else {
            continue;
        };
        let Some(prototype) = prototypes.get(mob_base.vob_id) else {
            warn!("No vob prototype for saved object vob({})", mob_base.vob_id);
            continue;
        };
        send_saved_prototype(&spawns, prototype, object.chest.as_ref(), transform);
    }
}

/// An npc spawn keeps the place name so `spawn_npc_entities` can apply the
/// routine waypoint, ground offset and model scale.
fn npc_spawn_request(npc: &SavedNpc, object: &SavedObject) -> SpawnNpc {
    let place = object
        .spawn_at_place
        .as_ref()
        .map(|place| place.place.clone());
    let exact = place.is_none();
    SpawnNpc {
        instance_id: npc.instance_id,
        location_name: place,
        transform: exact.then_some(object.transform),
        exact_transform: exact,
        overlay: object
            .overlay
            .as_ref()
            .and_then(|overlay| overlay.0.clone()),
    }
}

/// Placement of a saved object: the named waypoint/spot when present, otherwise
/// its stored transform.
fn saved_object_transform(
    object: &SavedObject,
    way_points: &WayPoints,
    spots: &Spots,
) -> Option<Transform> {
    let Some(place) = &object.spawn_at_place else {
        return Some(object.transform);
    };
    let transform = way_points
        .points
        .get(place.place.as_str())
        .or_else(|| spots.0.get(place.place.as_str()));
    let Some(transform) = transform else {
        warn!("No waypoint or spot({}) for saved object", place.place);
        return None;
    };
    Some(*transform)
}

/// Sends the prototype spawn message at `transform`, applying saved container
/// state to the mutable part of a container prototype.
fn send_saved_prototype(
    spawns: &WorldSpawnMessages,
    prototype: &VobPrototype,
    chest: Option<&SavedChest>,
    transform: Transform,
) {
    match prototype {
        VobPrototype::Static(spawn) => {
            let mut spawn = spawn.clone();
            spawn.tr = transform;
            spawns.static_models.send(spawn);
        }
        VobPrototype::Mover(spawn) => {
            let mut spawn = spawn.clone();
            spawn.tr = transform;
            spawns.movers.send(spawn);
        }
        VobPrototype::Item(spawn) => {
            let mut spawn = spawn.clone();
            spawn.transform = Some(transform);
            spawns.items.send(spawn);
        }
        VobPrototype::Mob(spawn) => {
            let mut mob = spawn.clone();
            mob.tr = transform;
            if let (Some(chest), MobSpawnKind::Container(container)) = (chest, &mut mob.kind) {
                container.items.clone_from(&chest.items);
                container.locked = chest.locked;
                container.cracked = chest.cracked;
            }
            spawns.mobs.send(mob);
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

/// The spawned player's origin is the model root (feet) and its capsule
/// collider is offset up by [`PLAYER_MODEL_ROOT_TO_CAPSULE_CENTER`].
///
/// The player is the `PC_HERO` instance, the `hero` instance global has no data
/// of its own (its stats come from `PC_HERO`).
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
    let talent_overlay = talent_overlay_for(&handle);

    let capsule_center = Vec3::Y * PLAYER_MODEL_ROOT_TO_CAPSULE_CENTER;
    commands.spawn((
        Name::new("PC_HERO"),
        Visibility::default(),
        CharacterController,
        CharacterMovementSettings::default(),
        CharacterCollisions::default(),
        GroundDetection {
            max_angle: PI / 3.0,
            max_distance: 0.2,
        },
        Collider::compound(vec![(
            capsule_center,
            Quat::IDENTITY,
            Collider::capsule(0.3, 1.2),
        )]),
        TransformInterpolation,
        player_transform.unwrap_or_else(default_player_transform),
        PlayerCharacter,
        GameNpc::new(handle),
        PlayerMeleeCombo::default(),
        talent_overlay,
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
    /// Script symbol name, matched by [`NpcSpawnNameFilter`].
    symbol_name: String,
    /// Localized display name, matched by [`NpcSpawnNameFilter`].
    display_name: Option<String>,
    handle: NpcHandle,
    body_model: String,
    body_texture: Option<String>,
    head_model: Option<String>,
    face_texture: Option<String>,
    hierarchy: Option<String>,
    model_scale: Option<Vec3>,
    model_fatness: Option<f32>,
    /// Active routine entry at spawn time; its waypoint takes precedence over
    /// the spawn location.
    routine_entry: Option<RoutineEntry>,
    location_name: Option<StringCanon>,
}

fn npc_spawn_script_data(
    scripts: &ScriptResource,
    spawn: &SpawnNpc,
    minutes_of_day: u32,
) -> Option<NpcSpawnScript> {
    let mut script = scripts.lock();
    let symbol_name = script
        .symbol_table()
        .symbol_name(spawn.instance_id.value())
        .unwrap_or_default();
    let display_name = script.npc_display_name(spawn.instance_id);
    let location_name = spawn.location_name.as_deref().map(StringCanon::new);
    let handle = script.npc_handle(spawn.instance_id)?;
    script.ensure_npc_routine(spawn.instance_id);
    let state = script.get_loaded_state();
    let instance = state.npc_instances.get(&spawn.instance_id)?;
    let instance = instance
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let routine_entry = instance.get_routine_entry(minutes_of_day).cloned();
    Some(NpcSpawnScript {
        symbol_name,
        display_name,
        handle,
        body_model: instance.visual.body_model.clone(),
        body_texture: instance.visual.body_texture.clone(),
        head_model: instance.visual.head_model.clone(),
        face_texture: instance.visual.face_texture.clone(),
        hierarchy: instance.visual.hierarchy.clone(),
        model_scale: instance.visual.model_scale,
        model_fatness: instance.visual.model_fatness,
        routine_entry,
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
    npc_filter: Res<NpcSpawnNameFilter>,
    position_filter: Res<SpawnPositionFilter>,
) {
    let _perf = perf_debug!();
    for spawn in spawns.read() {
        let Some(script) = npc_spawn_script_data(&gothic_scripts, spawn, game_time.hour() * 60)
        else {
            warn!("No instance state for npc({}) spawn", spawn.instance_id);
            continue;
        };
        instance_index.insert_npc_handle(script.handle.clone());
        if !npc_filter.allows(&script.symbol_name, script.display_name.as_deref()) {
            debug!(
                "Skipping npc({}) {} spawn, filtered by name",
                spawn.instance_id, script.symbol_name
            );
            continue;
        }

        let mut transform = if let Some(tr) = spawn.transform {
            tr
        } else {
            // Routine waypoint takes precedence over the spawn location,
            // spots take precedence over way points
            let name = script
                .routine_entry
                .as_ref()
                .map(|entry| entry.way_point.clone())
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
        if !position_filter.allows(transform.translation) {
            debug!(
                "Skipping npc({}) {} spawn, outside the spawn position filter",
                spawn.instance_id, script.symbol_name
            );
            continue;
        }

        let Some(body_model) = mesh_body_path(&vfs, &script.body_model) else {
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

        let npc_name = if script.symbol_name.is_empty() {
            spawn.instance_id.to_string()
        } else {
            script.symbol_name.clone()
        };
        let mut entity = commands.spawn((
            Name::new(npc_name),
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
        ));
        if let Some(entry) = script.routine_entry {
            entity.insert(entry);
        }
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
    vfs: Res<ZenginVfs>,
    mut instance_index: ResMut<InstanceIndex>,
    position_filter: Res<SpawnPositionFilter>,
) {
    let _perf = perf_debug!();
    for spawn in spawns.read() {
        let Some((handle, name)) = ({
            let mut script = gothic_scripts.lock();
            script.item_handle(spawn.instance_id).map(|handle| {
                let name = script
                    .symbol_table()
                    .symbol_name(spawn.instance_id.value())
                    .unwrap_or_else(|| spawn.instance_id.to_string());
                (handle, name)
            })
        }) else {
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
        let Some(waypoint_tr) = spawn.transform else {
            continue;
        };

        if !position_filter.allows(waypoint_tr.translation) {
            debug!(
                "Skipping item({}) spawn, outside the spawn position filter",
                spawn.instance_id
            );
            continue;
        }

        let model_handle = handles_map.get_model_handle(&asset_server, &model, None);

        commands.spawn((
            Name::new(name),
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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use bevy::prelude::*;

    use super::{
        NpcSpawnNameFilter, Spots, WayPoints, saved_object_from_prototype, saved_object_transform,
        send_saved_prototype,
    };
    use crate::save::format::{SavedChest, SavedObject, SavedObjectBuilder, SavedSpawnAtPlace};
    use crate::zengin::InstanceId;
    use crate::zengin::MobBase;
    use crate::zengin::script::state::SpawnItem;
    use crate::zengin::world_spawn::{
        ContainerSpawn, MobSpawnKind, SpawnMob, SpawnStaticModel, VobPrototype, WorldSpawnMessages,
    };

    #[test]
    fn empty_filter_allows_every_npc() {
        let filter = NpcSpawnNameFilter::default();
        assert!(filter.allows("vlk_449_lares", Some("Lares")));
    }

    #[test]
    fn filter_matches_symbol_or_display_name_substring() {
        let filter = filter_with(&["lares"]);
        assert!(filter.allows("vlk_449_lares", None));
        assert!(filter.allows("other", Some("Lares")));
        assert!(!filter.allows("vlk_449_milten", Some("Milten")));
    }

    #[test]
    fn added_filters_accumulate() {
        let mut filter = NpcSpawnNameFilter::default();
        filter.add(&["lares".to_string()]);
        filter.add(&["milten".to_string()]);
        assert!(filter.allows("vlk_449_lares", None));
        assert!(filter.allows("vlk_449_milten", None));
        assert!(!filter.allows("vlk_449_diego", None));
    }

    #[test]
    fn matching_is_case_insensitive() {
        let filter = filter_with(&["LaReS"]);
        assert!(filter.allows("vlk_449_lares", None));
        assert!(filter.allows("other", Some("Lares")));
    }

    fn filter_with(names: &[&str]) -> NpcSpawnNameFilter {
        let mut filter = NpcSpawnNameFilter::default();
        let names: Vec<String> = names.iter().map(|name| (*name).to_string()).collect();
        filter.add(&names);
        filter
    }

    fn saved_object(transform: Transform) -> SavedObject {
        SavedObjectBuilder::new(transform).build()
    }

    fn empty_world_spawns() -> WorldSpawnMessages {
        WorldSpawnMessages {
            world_mesh: default(),
            static_models: default(),
            mobs: default(),
            movers: default(),
            items: default(),
            objects: default(),
        }
    }

    #[test]
    fn saved_object_uses_spawn_at_place_over_transform() {
        let way_points = WayPoints {
            points: HashMap::from([("WP".to_string(), Transform::from_xyz(1.0, 2.0, 3.0))]),
        };
        let spots = Spots::default();
        let transform = Transform::from_xyz(9.0, 9.0, 9.0);
        let mut object = saved_object(transform);

        assert_eq!(
            saved_object_transform(&object, &way_points, &spots),
            Some(transform)
        );

        object.spawn_at_place = Some(SavedSpawnAtPlace {
            place: "WP".to_string(),
        });
        assert_eq!(
            saved_object_transform(&object, &way_points, &spots),
            Some(Transform::from_xyz(1.0, 2.0, 3.0))
        );

        object.spawn_at_place = Some(SavedSpawnAtPlace {
            place: "MISSING".to_string(),
        });
        assert!(saved_object_transform(&object, &way_points, &spots).is_none());
    }

    #[test]
    fn prototype_plan_matches_component_payloads() {
        let static_proto = VobPrototype::Static(SpawnStaticModel {
            vob_id: 4,
            tr: Transform::from_xyz(1.0, 2.0, 3.0),
            archetype: "mesh".to_string(),
            collides: true,
        });
        let object = saved_object_from_prototype(&static_proto).expect("static plan");
        assert_eq!(object.transform, Transform::from_xyz(1.0, 2.0, 3.0));
        assert_eq!(object.mob_base.as_ref().map(|base| base.vob_id), Some(4));
        assert!(object.static_object.is_some());
        assert!(object.chest.is_none());

        let item = InstanceId::new(3).expect("item id");
        let container = VobPrototype::Mob(SpawnMob {
            base: MobBase {
                vob_id: 7,
                vob_name: "CHEST".to_string(),
                focus_name: String::new(),
            },
            tr: Transform::IDENTITY,
            archetype: String::new(),
            visual_hidden: false,
            collides: false,
            scheme: "CHEST".to_string(),
            visual_stem: String::new(),
            state_num: 1,
            kind: MobSpawnKind::Container(ContainerSpawn {
                items: vec![(item, 2)],
                locked: true,
                key: "KEY".to_string(),
                pick_string: "LR".to_string(),
                cracked: false,
            }),
        });
        let object = saved_object_from_prototype(&container).expect("chest plan");
        assert!(object.static_object.is_none());
        assert_eq!(object.mob_base.as_ref().map(|base| base.vob_id), Some(7));
        let chest = object.chest.as_ref().expect("chest payload");
        assert!(chest.locked);
        assert_eq!(chest.items, vec![(item, 2)]);
        assert!(!chest.cracked);
    }

    #[test]
    fn item_prototype_without_transform_is_not_planned() {
        let item = InstanceId::new(3).expect("item id");
        let prototype = VobPrototype::Item(SpawnItem {
            instance_id: item,
            transform: None,
        });
        assert!(saved_object_from_prototype(&prototype).is_none());
    }

    #[test]
    fn saved_chest_overrides_container_prototype_state() {
        let spawns = empty_world_spawns();
        let prototype = VobPrototype::Mob(SpawnMob {
            base: MobBase {
                vob_id: 7,
                vob_name: "CHEST".to_string(),
                focus_name: String::new(),
            },
            tr: Transform::IDENTITY,
            archetype: String::new(),
            visual_hidden: false,
            collides: true,
            scheme: "CHEST".to_string(),
            visual_stem: String::new(),
            state_num: 1,
            kind: MobSpawnKind::Container(ContainerSpawn {
                items: Vec::new(),
                locked: true,
                key: "KEY".to_string(),
                pick_string: "LR".to_string(),
                cracked: false,
            }),
        });
        let item = InstanceId::new(3).expect("item id");
        let chest = SavedChest {
            locked: false,
            items: vec![(item, 2)],
            cracked: true,
        };

        let transform = Transform::from_xyz(1.0, 2.0, 3.0);
        send_saved_prototype(&spawns, &prototype, Some(&chest), transform);

        let mobs = spawns.mobs.take();
        assert_eq!(mobs.len(), 1);
        assert_eq!(mobs[0].tr, transform);
        let MobSpawnKind::Container(container) = &mobs[0].kind else {
            panic!("expected a container prototype");
        };
        assert!(!container.locked);
        assert!(container.cracked);
        assert_eq!(container.items, vec![(item, 2)]);
    }
}
