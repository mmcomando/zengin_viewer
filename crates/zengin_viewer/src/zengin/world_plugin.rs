use crate::gameplay::character::{
    CharacterCollisions, CharacterController, CharacterMovementSettings, GroundDetection,
    PlayerPhysicsBodyMarker,
};
use crate::gameplay::distance::ClassifyDistance;
use crate::gameplay::game_time::GameTime;
use crate::gameplay::objects::{NpcModel, PlayerCharacter};
use crate::graphics::animation::{MovementAnimations, NpcAnimations};
use crate::gui_debug::toggle_visibility::{NpcVisibility, StaticMesh, WorldMesh};
use crate::zengin::GameNpc;
use crate::zengin::common::ZenGinModel;
use crate::zengin::npc_routines::{WayPoints, WeaponMeshPaths};
use crate::zengin::objects_to_entities::GameObjectSpawnEntities;
use crate::zengin::script::component::ScriptResource;
use crate::zengin::world::load_zengin_world_data;
use crate::zengin::zengin_resources::{
    DYNAMIC_OBJECT, MaterialHandles, STATIC_OBJECT, ZenGinInsertResources, ZenGinModelComponent,
};
use avian3d::math::PI;
use avian3d::prelude::*;
use bevy::{color::palettes::tailwind, prelude::*};

/// Loads the zengin world data and spawns the world and player entities.
pub struct ZenGinWorldPlugin;

impl Plugin for ZenGinWorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ZenGinInsertResources);
        app.add_plugins(GameObjectSpawnEntities);

        app.add_systems(Startup, spawn_world);
    }
}

fn spawn_world(
    mut commands: Commands,
    mut handles_map: ResMut<MaterialHandles>,
    asset_server: Res<AssetServer>,
    gothic_script: Res<ScriptResource>,
    game_time: Res<GameTime>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let _span = info_span!("spawn_world").entered();
    println!("\n-----SCRIPTS VM-----\n");
    let mut gothic_script = gothic_script.lock();
    let loaded_state = gothic_script.get_loaded_state();
    println!("\n-----LOAD WORLD ZEN DATA-----\n");
    let mut world_data = load_zengin_world_data(
        "/_WORK/DATA/WORLDS/NEWWORLD/NEWWORLD.ZEN",
        loaded_state,
        game_time.hour(),
    );
    if false {
        let world_data_oldw = load_zengin_world_data(
            "/_WORK/DATA/WORLDS/OLDWORLD/OLDWORLD.ZEN",
            loaded_state,
            game_time.hour(),
        );
        world_data
            .light_instances
            .extend(world_data_oldw.light_instances);
    }

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
    // Seed npc positions so distance queries work before the first ECS update
    {
        let positions = world_data
            .npcs
            .iter()
            .map(|npc| (npc.npc_intance_id, npc.body_tr.translation))
            .collect();
        if let Ok(mut live) = gothic_script.npc_live_state().lock() {
            live.positions = positions;
        }
    }
    commands.insert_resource(WayPoints { points: way_points });
    commands.insert_resource(WeaponMeshPaths(std::mem::take(
        &mut world_data.item_mesh_paths,
    )));

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

    for npc in &world_data.npcs {
        // if npc.npc_intance_id.value() != 12562 {
        //     continue;
        // }
        // println!("spawn_npc({})", npc.npc_intance_id);
        commands.spawn((
            Visibility::default(),
            GameNpc {
                npc_instance_id: npc.npc_intance_id,
            },
            NpcModel {
                hierarchy: npc.hierarchy.clone(),
                body_model: npc.body_model.clone(),
                body_texture: npc.body_texture.clone(),
                head_model: npc.head_model.clone(),
                head_texture: npc.head_texture.clone(),
                armor_model: npc.armor_model.clone(),
                animation_scheme: npc.animation_scheme.clone(),
                stand_animation: npc.stand_animation.clone(),
                model_fatness: npc.model_fatness,
            },
            NpcAnimations {
                // animation: Some(
                //     // "zengin://_WORK/DATA/ANIMS/_COMPILED/HUMANS-S_WATCHFIGHT.MAN".to_string(),
                //     "zengin://_WORK/DATA/ANIMS/_COMPILED/HUMANS-S_FIRE_VICTIM.MAN".to_string(),
                // ),
                animation: None,
                movement_animations: None,
            },
            npc.body_tr,
            NpcVisibility::default(),
        ));
    }
    for instance in &world_data.items {
        let model_handle = handles_map.get_model_handle(&asset_server, &instance.model, None);
        commands.spawn((
            ClassifyDistance::default(),
            ZenGinModelComponent {
                model_handle: model_handle.clone(),
                ..default()
            },
            Visibility::default(),
            instance.tr,
        ));
    }
    for instance in &world_data.static_models {
        let model_handle = handles_map.get_model_handle(&asset_server, &instance.archetype, None);
        commands.spawn((
            ClassifyDistance::default(),
            ZenGinModelComponent {
                model_handle: model_handle.clone(),
                convex_colider: true,
                ..default()
            },
            Visibility::default(),
            instance.tr,
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

    for x in -4..4 {
        for z in -4..4 {
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

    // Player collider
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
            // TODO hardcoded player instance id
            npc_instance_id: gothic_script
                .instance_id(1620)
                .expect("player instance id should be a valid instance symbol"),
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
        NpcAnimations {
            animation: None,
            movement_animations: Some(MovementAnimations {
                idle: "zengin://_WORK/DATA/ANIMS/_COMPILED/HUMANS-S_RUN.MAN".to_string(),
                move_start: Some(
                    "zengin://_WORK/DATA/ANIMS/_COMPILED/HUMANS-T_RUN_2_RUNL.MAN".to_string(),
                ),
                move_loop: "zengin://_WORK/DATA/ANIMS/_COMPILED/HUMANS-S_RUNL.MAN".to_string(),
                move_stop: Some(
                    "zengin://_WORK/DATA/ANIMS/_COMPILED/HUMANS-T_RUNL_2_RUN.MAN".to_string(),
                ),
            }),
        },
    ));
}
