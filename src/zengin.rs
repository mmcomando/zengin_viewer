pub mod common;
pub mod loaders;
pub mod macros;
pub mod script;
pub mod visual;
pub mod world;

use std::sync::Arc;

use crate::character::{
    CharacterCollisions, CharacterController, CharacterMovementSettings, GroundDetection,
};
use crate::game::objects::GameNpc;
use crate::game::objects_to_entities::GameObjectSpawnEntities;
use crate::toggle_visibility::{NpcVisibility, StaticMesh, WorldMesh};
use crate::zengin::common::{ZenGinModel, gothic2_dir};
use crate::zengin::script::parse::*;
use crate::zengin::script::script_vm::ScriptVM;
use crate::zengin::world::load_zengin_world_data;
use crate::zengin_resources::{
    DYNAMIC_OBJECT, MaterialHandles, STATIC_OBJECT, ZenGinInsertResources, ZenGinModelComponent,
};
use avian3d::math::PI;
use avian3d::prelude::*;
use bevy::{color::palettes::tailwind, prelude::*};

#[derive(Default)]
pub struct ZenGinWorldPlugin;

impl Plugin for ZenGinWorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ZenGinInsertResources);
        app.add_plugins(GameObjectSpawnEntities);

        app.add_systems(Startup, spawn_world);
    }
}

#[derive(Default, Component)]
pub struct PlayerMarker;

fn get_zen_gin_world_init_state() -> crate::zengin::script::script_vm::State {
    let _span = info_span!("InitScripts",).entered();
    let path_str = gothic2_dir() + "/_work/Data/Scripts/_compiled/GOTHIC.DAT";
    let dat_data = parse_dat(&path_str).unwrap();
    let dat_data = Arc::from(dat_data);
    let mut vm_state = crate::zengin::script::script_vm::State::new(dat_data.clone());
    let script_vm = ScriptVM::new(dat_data.clone());

    script_vm.initialize_variables(&mut vm_state);
    script_vm.interpret_script_function(&mut vm_state, "startup_newworld");
    script_vm.instantiate_npc_routines(&mut vm_state);
    vm_state
}

fn spawn_world(
    mut commands: Commands,
    mut handles_map: ResMut<MaterialHandles>,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let _span = info_span!("spawn_world").entered();
    println!("\n-----SCRIPTS VM-----\n");
    let vm_state = get_zen_gin_world_init_state();
    println!("\n-----LOAD WORLD ZEN DATA-----\n");
    let mut world_data =
        load_zengin_world_data("/_WORK/DATA/WORLDS/NEWWORLD/NEWWORLD.ZEN", &vm_state);
    if false {
        let world_data_oldw =
            load_zengin_world_data("/_WORK/DATA/WORLDS/OLDWORLD/OLDWORLD.ZEN", &vm_state);
        world_data
            .light_instances
            .extend(world_data_oldw.light_instances);
    }

    println!("\n----CREATE ENGINE ENTITIES------\n");

    let world_model_handle: Handle<ZenGinModel> = asset_server.add(world_data.world_model);

    commands.spawn((
        Visibility::default(),
        ZenGinModelComponent {
            model_handle: world_model_handle,
            trimesh_collider: true,
            ..default()
        },
        Transform::IDENTITY,
        WorldMesh::default(),
    ));

    for npc in &world_data.npcs {
        commands.spawn((
            Visibility::default(),
            GameNpc {
                tr: Transform::IDENTITY,
                body_model: npc.body_model.clone(),
                body_texture: npc.body_texture.clone(),
                head_model: npc.head_model.clone(),
                head_texture: npc.head_texture.clone(),
                armor_model: npc.armor_model.clone(),
            },
            npc.body_tr,
            NpcVisibility::default(),
        ));
    }
    for instance in &world_data.items {
        let model_handle = handles_map.get_model_handle(&asset_server, &instance.model);
        commands.spawn((
            ZenGinModelComponent {
                model_handle: model_handle.clone(),
                ..default()
            },
            Visibility::default(),
            instance.tr,
        ));
    }
    for instance in &world_data.static_models {
        let model_handle = handles_map.get_model_handle(&asset_server, &instance.archetype);
        commands.spawn((
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

    // Player
    commands.spawn((
        Visibility::default(),
        PlayerMarker,
        CharacterController,
        CharacterMovementSettings::default(),
        CharacterCollisions::default(),
        GroundDetection {
            // Use a slightly smaller capsule for shape casts used for ground detection
            cast_shape: Some(Collider::capsule(0.499, 0.8)),
            max_angle: PI / 3.0,
            max_distance: 0.2,
        },
        Collider::capsule(0.4, 1.0),
        // Mesh3d(meshes.add(Capsule3d::new(0.5, 0.8))),
        // MeshMaterial3d(materials.add(Color::srgb(0.8, 0.7, 0.6))),
        Transform::from_xyz(0.0, 1.5, 0.0),
        TransformInterpolation,
        GameNpc {
            tr: Transform::from_xyz(0.0, -0.9, 0.0),
            body_model: "zengin://_WORK/DATA/ANIMS/_COMPILED/HUM_BODY_NAKED0.MDM".to_string(),
            body_texture: Some(
                "zengin://_WORK/DATA/TEXTURES/_COMPILED/HUM_BODY_NAKED_V9_C0-C.TEX".to_string(),
            ),
            head_model: Some("zengin://_WORK/DATA/ANIMS/_COMPILED/HUM_HEAD_PONY.MMB".to_string()),
            head_texture: Some(
                "zengin://_WORK/DATA/TEXTURES/_COMPILED/HUM_HEAD_V18_C0-C.TEX".to_string(),
            ),
            armor_model: None,
        },
    ));
}
