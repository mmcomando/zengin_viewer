use std::f32::consts::PI;

use crate::gameplay::character::{
    CharacterCollisions, CharacterController, CharacterMovementSettings, GroundDetection,
    PlayerPhysicsBodyMarker,
};
use crate::gameplay::distance::ClassifyDistance;
use crate::gameplay::game_time::GameTime;
use crate::gameplay::objects::{NpcModel, PlayerCharacter};
use crate::graphics::NpcAnimations;
use crate::gui_debug::{NpcVisibility, StaticMesh, WorldMesh};
use crate::zengin::common::get_full_texture_path;
use crate::zengin::loaders::vfs::{
    item_model_path, mesh_model_path, normalize_mesh_name, resolve_stand_animation,
};
use crate::zengin::npc_routines::{AnimationFiles, WayPoints};
use crate::zengin::objects_to_entities::GameObjectSpawnEntities;
use crate::zengin::resources::{
    CachedAssetHandles, DYNAMIC_OBJECT, NamedModels, SELECTABLE, STATIC_OBJECT,
    ZenGinInsertResources, ZenGinModelComponent,
};
use crate::zengin::script::bridge::ScriptChangesApplied;
use crate::zengin::script::component::ScriptResource;
use crate::zengin::script::manager::ScriptManager;
use crate::zengin::script::state::{LoadedState, SpawnItem, SpawnNpc};
use crate::zengin::script::string_value::StringCanon;
use crate::zengin::visual::model::ZenGinModel;
use crate::zengin::visual::world::load_zengin_world_data;
use crate::zengin::waypoint_graph::{Spots, WaypointGraph, spawn_waypoint_entities};
use crate::zengin::{GameChest, GameItem, GameNpc, ZenginVfs};
use avian3d::prelude::*;
use bevy::{color::palettes::tailwind, prelude::*};

/// Loads the zengin world data and spawns the world and player entities.
pub struct ZenGinWorldPlugin;

impl Plugin for ZenGinWorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ZenGinInsertResources);
        app.add_plugins(GameObjectSpawnEntities);

        app.init_resource::<AnimationFiles>();
        app.init_resource::<WayPoints>();
        app.init_resource::<Spots>();
        app.init_resource::<WaypointGraph>();

        app.add_message::<SpawnWorld>();
        app.add_systems(Startup, request_initial_world);
        app.add_systems(Update, spawn_world.before(ScriptChangesApplied));
        app.add_systems(
            Update,
            (spawn_npc_entities, spawn_item_entities)
                .chain()
                .after(ScriptChangesApplied),
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
}

fn request_initial_world(mut spawn_world: MessageWriter<SpawnWorld>) {
    spawn_world.write(SpawnWorld {
        world_name: "NEWWORLD/NEWWORLD".to_string(),
        clean_spawn: true,
    });
}

fn spawn_world(
    mut requests: MessageReader<SpawnWorld>,
    mut commands: Commands,
    mut handles_map: ResMut<CachedAssetHandles>,
    mut named_models: ResMut<NamedModels>,
    asset_server: Res<AssetServer>,
    vfs: Res<ZenginVfs>,
    gothic_script: Res<ScriptResource>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut spawn_item: MessageWriter<SpawnItem>,
) {
    for SpawnWorld {
        world_name,
        clean_spawn,
    } in requests.read()
    {
        spawn_world_data(
            world_name,
            *clean_spawn,
            &mut commands,
            &mut handles_map,
            &mut named_models,
            &asset_server,
            &vfs,
            &gothic_script,
            &mut materials,
            &mut meshes,
            &mut spawn_item,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_world_data(
    world_name: &str,
    clean_spawn: bool,
    commands: &mut Commands,
    handles_map: &mut CachedAssetHandles,
    named_models: &mut NamedModels,
    asset_server: &Res<AssetServer>,
    vfs: &ZenginVfs,
    gothic_script: &ScriptResource,
    materials: &mut Assets<StandardMaterial>,
    meshes: &mut Assets<Mesh>,
    spawn_item: &mut MessageWriter<SpawnItem>,
) {
    let _span = info_span!("spawn_world").entered();
    println!("\n-----SCRIPTS VM-----\n");
    let mut gothic_script = gothic_script.lock();
    let loaded_state = gothic_script.loaded_state();
    println!("\n-----LOAD WORLD ZEN DATA-----\n");
    let world_path = format!("/_WORK/DATA/WORLDS/{world_name}.ZEN");
    let mut world_data = load_zengin_world_data(&world_path, loaded_state, spawn_item);
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
    gothic_script.load_world(world_name, clean_spawn);

    commands.insert_resource(WayPoints { points: way_points });
    commands.insert_resource(Spots(std::mem::take(&mut world_data.spots)));
    commands.insert_resource(AnimationFiles(std::mem::take(&mut world_data.anim_files)));
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

    for chest in world_data.chests {
        let model_handle = handles_map.get_model_handle(asset_server, &chest.archetype, None);
        commands.spawn((
            ClassifyDistance::default(),
            GameChest {
                name: chest.name,
                locked: chest.locked,
                items: chest.items,
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

    for instance in world_data.light_instances {
        if true {
            // For now ignore lights for performance, also it seems that most lights are for baking light data, not for rendering?
            continue;
        }
        let tr = Transform::from_translation(instance.pos).with_rotation(instance.rot);
        commands.spawn((
            Visibility::default(),
            PointLight {
                color: Color::from(tailwind::ORANGE_300),
                intensity: light_consts::lumens::LUMENS_PER_HALOGEN_WATTS * 5000.0,
                range: 5.0,
                ..default()
            },
            tr,
        ));
    }

    for x in -2..2 {
        for z in -2..2 {
            commands.spawn((
                RigidBody::Dynamic,
                Collider::cuboid(1.0, 1.0, 1.0),
                AngularVelocity(Vec3::new(2.5, 3.5, 1.5)),
                Mesh3d(meshes.add(Cuboid::from_length(1.0))),
                MeshMaterial3d(materials.add(Color::srgb_u8(124, 144, 255))),
                CollisionLayers::from_bits(DYNAMIC_OBJECT, STATIC_OBJECT | DYNAMIC_OBJECT),
                #[allow(clippy::cast_precision_loss)]
                Transform::from_xyz(-30.0 + x as f32 * 5.0, 30.0, z as f32 * 5.0),
            ));
        }
    }

    if !gothic_script.has_player_instance() {
        spawn_player(commands, &mut gothic_script);
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
        .find(|(_, instance)| instance.name.eq_ignore_ascii_case("ITRW_ARROW"))
        .and_then(|(_, instance)| mesh_model_path(vfs, instance.chosen_visual()))
        .map(|path| handles.get_model_handle(asset_server, &path, None));
    if named_models.item_arrow.is_none() {
        warn!("No ITRW_ARROW item instance found, arrows will have no visual");
    }
}

/// Spawns the player entity: a physics collider with the `PC_HERO` visual as a
/// child. The player is the `PC_HERO` instance, the `hero` instance global has
/// no data of its own (its stats come from `PC_HERO`).
fn spawn_player(commands: &mut Commands, gothic_script: &mut ScriptManager) {
    let player_instance = gothic_script
        .symbol_instance("pc_hero")
        .expect("player instance pc_hero should be a valid instance symbol");
    gothic_script.set_player_instance(player_instance);
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
        Transform::from_xyz(-97.0, 6.0, -7.0),
    ));

    // Player visual
    player_collider.with_child((
        Visibility::default(),
        Transform::from_xyz(0.0, -1.0, 0.0),
        PlayerCharacter,
        GameNpc {
            npc_instance_id: player_instance,
        },
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
            stand_animation: None,
            animation_scheme: None,
            model_fatness: None,
            armor_model: Some("zengin://_WORK/DATA/ANIMS/_COMPILED/ARMOR_LESTER.MDM".to_string()),
        },
        NpcAnimations { animation: None },
    ));
}

/// Data needed to spawn one npc entity, extracted from the script state in a
/// single lock.
struct NpcSpawnScript {
    body_model: String,
    body_texture: Option<String>,
    head_model: Option<String>,
    face_texture: Option<String>,
    /// Visual chosen for the equipped armor, when the npc has one.
    armor_visual: Option<String>,
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
    let state = script.get_loaded_state();
    let instance = state.npc_instances.get(&spawn.instance_id)?;
    let routine_way_point = instance
        .get_routine_entry(minutes_of_day)
        .map(|entry| entry.way_point.clone());
    let armor_visual = instance.armor_model.as_ref().and_then(|name| {
        state
            .items_instances
            .values()
            .find(|item| &item.name == name)
            .map(|item| item.chosen_visual().to_string())
    });
    Some(NpcSpawnScript {
        body_model: instance.body_model.clone(),
        body_texture: instance.body_texture.clone(),
        head_model: instance.head_model.clone(),
        face_texture: instance.face_texture.clone(),
        armor_visual,
        hierarchy: instance.hierarchy.clone(),
        model_scale: instance.model_scale,
        model_fatness: instance.model_fatness,
        routine_way_point,
        location_name,
    })
}

/// Spawns a full npc entity for every requested npc, resolving its placement
/// from the routine/spawn location and its visuals from the script state and
/// the shared vfs.
fn spawn_npc_entities(
    mut spawns: MessageReader<SpawnNpc>,
    mut commands: Commands,
    gothic_scripts: Res<ScriptResource>,
    vfs: Res<ZenginVfs>,
    way_points: Res<WayPoints>,
    spots: Res<Spots>,
    game_time: Res<GameTime>,
) {
    for spawn in spawns.read() {
        let Some(script) = npc_spawn_script_data(&gothic_scripts, spawn, game_time.hour() * 60)
        else {
            warn!("No instance state for npc({}) spawn", spawn.instance_id);
            continue;
        };

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
        // Place the visual on the ground, matching the engine placement
        transform.translation.y -= 0.5;
        if let Some(scale) = script.model_scale {
            transform.scale = scale;
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
        let armor_model = script.armor_visual.as_deref().and_then(|visual| {
            let path = mesh_model_path(&vfs, visual);
            if path.is_none() {
                warn!(
                    "No mesh for npc({}) armor visual({})",
                    spawn.instance_id, visual
                );
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

        commands.spawn((
            Visibility::default(),
            GameNpc {
                npc_instance_id: spawn.instance_id,
            },
            NpcModel {
                hierarchy,
                body_model,
                body_texture: script.body_texture.as_deref().map(get_full_texture_path),
                head_model,
                head_texture: script.face_texture.as_deref().map(get_full_texture_path),
                armor_model,
                animation_scheme,
                stand_animation,
                model_fatness: script.model_fatness,
            },
            NpcAnimations::default(),
            transform,
            NpcVisibility::default(),
        ));
    }
}

/// Spawns an item entity, resolving its model from the script item instance.
fn spawn_item_entities(
    mut spawns: MessageReader<SpawnItem>,
    mut commands: Commands,
    mut handles_map: ResMut<CachedAssetHandles>,
    asset_server: Res<AssetServer>,
    gothic_scripts: ResMut<ScriptResource>,
    way_points: Res<WayPoints>,
    spots: Res<Spots>,

    vfs: Res<ZenginVfs>,
) {
    for spawn in spawns.read() {
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
            GameItem {
                item_instance_id: spawn.instance_id,
            },
        ));
    }
}
