use std::{
    f32::consts::{FRAC_PI_4, PI},
    ffi::CString,
};

use bevy::anti_alias::smaa::Smaa;
use bevy::{
    asset::RenderAssetUsages,
    camera_controller::free_camera::{FreeCamera, FreeCameraPlugin, FreeCameraState},
    color::palettes::css::*,
    color::palettes::tailwind,
    mesh::Indices,
    mesh::PrimitiveTopology,
    prelude::*,
};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        // Plugin that enables FreeCamera functionality
        .add_plugins(FreeCameraPlugin)
        // Example code plugins
        .add_plugins((CameraPlugin, CameraSettingsPlugin, ScenePlugin))
        .run();
}

// Plugin that spawns the camera.
struct CameraPlugin;
impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_camera);
    }
}

fn print_nodes(node: *const ZenKitCAPI_sys::ZkVfsNode, level: u8) {
    use std::ffi::CStr;
    use std::os::raw::c_void;
    unsafe {
        let name_cstr = ZenKitCAPI_sys::ZkVfsNode_getName(node);
        let name_cstr = CStr::from_ptr(name_cstr);
        let name = String::from_utf8_lossy(name_cstr.to_bytes()).to_string();
        for _i in 0..level {
            print!(" ");
        }
        println!("{name}");

        extern "C" fn callback(
            ctx: *mut c_void,
            node: *const ZenKitCAPI_sys::ZkVfsNode,
        ) -> ZenKitCAPI_sys::ZkBool {
            let level: u8 = ctx as u8;
            print_nodes(node, level + 1);
            0
        }

        ZenKitCAPI_sys::ZkVfsNode_enumerateChildren(node, Some(callback), level as *mut c_void);
    }
}
fn spawn_camera(mut commands: Commands) {
    // ambient light
    // ambient lights' brightnesses are measured in candela per meter square, calculable as (color * brightness)
    commands.insert_resource(GlobalAmbientLight {
        color: WHITE.into(),
        brightness: 200.0,
        ..default()
    });
    commands.spawn((
        // AmbientLight {
        //     color: Color::linear_rgb(1.0, 1.0, 1.0),
        //     brightness: 1.0,
        //     affects_lightmapped_meshes: true,
        // },
        Smaa::default(),
        Camera3d::default(),
        Transform::from_xyz(0.0, 1.0, 0.0).looking_to(Vec3::X, Vec3::Y),
        // This component stores all camera settings and state, which is used by the FreeCameraPlugin to
        // control it. These properties can be changed at runtime, but beware the controller system is
        // constantly using and modifying those values unless the enabled field is false.
        FreeCamera {
            sensitivity: 0.2,
            friction: 25.0,
            walk_speed: 3.0,
            run_speed: 9.0,
            ..default()
        },
    ));
}

// Plugin that handles camera settings controls and information text
struct CameraSettingsPlugin;
impl Plugin for CameraSettingsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostStartup, spawn_text)
            // .add_systems(Update, update_cameras)
            .add_systems(Update, (update_camera_settings, update_text));
    }
}

#[derive(Component)]
struct InfoText;

fn spawn_text(mut commands: Commands, free_camera_query: Query<&FreeCamera>) {
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: px(-16),
            left: px(12),
            ..default()
        },
        children![Text::new(format!(
            "{}",
            free_camera_query.single().unwrap()
        ))],
    ));
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            bottom: px(12),
            left: px(12),
            ..default()
        },
        children![Text::new(concat![
            "Z/X: decrease/increase sensitivity\n",
            "C/V: decrease/increase friction\n",
            "F/G: decrease/increase scroll factor\n",
            "B: enable/disable controller",
        ]),],
    ));

    // Mutable text marked with component
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: px(12),
            right: px(12),
            ..default()
        },
        children![(InfoText, Text::new(""))],
    ));
}

// fn update_cameras(camera_query: Query<(Entity, &mut Camera3d)>) {
//     for (entity_id, _camera) in camera_query.iter() {
//         println!("Entity({:?})", entity_id);
//     }
//     // let (entity, mut camera) = camera_query.single_mut().unwrap();
// }
fn update_camera_settings(
    mut camera_query: Query<(&mut FreeCamera, &mut FreeCameraState)>,
    input: Res<ButtonInput<KeyCode>>,
) {
    let (mut free_camera, mut free_camera_state) = camera_query.single_mut().unwrap();

    if input.pressed(KeyCode::KeyZ) {
        free_camera.sensitivity = (free_camera.sensitivity - 0.005).max(0.005);
    }
    if input.pressed(KeyCode::KeyX) {
        free_camera.sensitivity += 0.005;
    }
    if input.pressed(KeyCode::KeyC) {
        free_camera.friction = (free_camera.friction - 0.2).max(0.0);
    }
    if input.pressed(KeyCode::KeyV) {
        free_camera.friction += 0.2;
    }
    if input.pressed(KeyCode::KeyF) {
        free_camera.scroll_factor = (free_camera.scroll_factor - 0.02).max(0.02);
    }
    if input.pressed(KeyCode::KeyG) {
        free_camera.scroll_factor += 0.02;
    }
    if input.just_pressed(KeyCode::KeyB) {
        free_camera_state.enabled = !free_camera_state.enabled;
    }
}

fn update_text(
    mut text_query: Query<&mut Text, With<InfoText>>,
    camera_query: Query<(&FreeCamera, &FreeCameraState)>,
) {
    let mut text = text_query.single_mut().unwrap();

    let (free_camera, free_camera_state) = camera_query.single().unwrap();

    text.0 = format!(
        "Enabled: {},\nSensitivity: {:.03}\nFriction: {:.01}\nScroll factor: {:.02}\nWalk Speed: {:.02}\nRun Speed: {:.02}\nSpeed: {:.02}",
        free_camera_state.enabled,
        free_camera.sensitivity,
        free_camera.friction,
        free_camera.scroll_factor,
        free_camera.walk_speed,
        free_camera.run_speed,
        free_camera_state.velocity.length(),
    );
}

// Plugin that spawns the scene and lighting.
struct ScenePlugin;
impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (spawn_lights, spawn_world));
    }
}

fn spawn_lights(mut commands: Commands) {
    // Main light
    commands.spawn((
        PointLight {
            color: Color::from(tailwind::ORANGE_300),
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(0.0, 3.0, 0.0),
    ));
    // Light behind wall
    commands.spawn((
        PointLight {
            color: Color::WHITE,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(-3.5, 3.0, 0.0),
    ));
    // Light under floor
    commands.spawn((
        PointLight {
            color: Color::from(tailwind::RED_300),
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(0.0, -0.5, 0.0),
    ));
}

fn get_wolrld_pos(gothic_pos: ZenKitCAPI_sys::ZkVec3f) -> Vec3 {
    let pos = ZkVec3f_to_Vec3(gothic_pos) / 100.0;
    // pos.x = -pos.x;
    return pos;
}

#[allow(non_snake_case)]
fn ZkVec3f_to_Vec3(data: ZenKitCAPI_sys::ZkVec3f) -> Vec3 {
    Vec3 {
        x: -data.x,
        y: data.y,
        z: data.z,
    }
}

#[allow(non_snake_case)]
fn ZkVec2f_to_Vec2(data: ZenKitCAPI_sys::ZkVec2f) -> Vec2 {
    Vec2 {
        x: data.x,
        y: data.y,
    }
}

fn create_gothic_world_mesh() -> Mesh {
    let mut indices: Vec<u32> = Vec::new();
    let mut vertices: Vec<Vec3> = Vec::new();
    let mut uvs: Vec<Vec2> = Vec::new();
    let mut normals: Vec<Vec3> = Vec::new();
    let mut colors: Vec<Vec4> = Vec::new();

    unsafe {
        let vfs = ZenKitCAPI_sys::ZkVfs_new();
        let file = CString::from(
            c"/media/MM_HDD_DATA/SteamLibrary/steamapps/common/Gothic II/Data/Worlds.vdf",
        );
        ZenKitCAPI_sys::ZkVfs_mountDiskHost(
            vfs,
            file.as_ptr(),
            ZenKitCAPI_sys::ZkVfsOverwriteBehavior::ZkVfsOverwriteBehavior_ALL,
        );

        let node = ZenKitCAPI_sys::ZkVfs_getRoot(vfs);
        print_nodes(node, 0);

        let world_node = ZenKitCAPI_sys::ZkVfs_resolvePath(
            vfs,
            CString::from(c"/_WORK/DATA/WORLDS/NEWWORLD/NEWWORLD.ZEN").as_ptr(),
        );
        println!("world_node {:?}", world_node);

        let world_read = ZenKitCAPI_sys::ZkVfsNode_open(world_node);
        println!("world_read {:?}", world_read);
        let world = ZenKitCAPI_sys::ZkWorld_load(world_read);
        println!("world {:?}", world);

        let mesh = ZenKitCAPI_sys::ZkWorld_getMesh(world);
        println!("mesh {:?}", mesh);

        let positions_count = ZenKitCAPI_sys::ZkMesh_getPositionCount(mesh);
        println!("Positions({positions_count}):");
        for position_index in 0..positions_count {
            // let position = ZenKitCAPI_sys::ZkMesh_getPosition(mesh, position_index);
            // // println!(" {position_index}) {position:?}");
            // vertices.push(get_wolrld_pos(position));
        }

        // uvs.set_len(vertices.len());
        // normals.set_len(vertices.len());

        let polygons_count = ZenKitCAPI_sys::ZkMesh_getPolygonCount(mesh);
        println!("Polygons({polygons_count}):");
        for polygon_index in 0..polygons_count {
            let polygon = ZenKitCAPI_sys::ZkMesh_getPolygon(mesh, polygon_index);

            if ZenKitCAPI_sys::ZkPolygon_getIsPortal(polygon) == 1 {
                println!("Skip polygon({polygon_index}) it is portal");
                continue;
            }

            let mut indices_count: ZenKitCAPI_sys::ZkSize = 0;
            let indices_ptr = ZenKitCAPI_sys::ZkPolygon_getPositionIndices(
                polygon,
                mesh,
                &mut indices_count as *mut _,
            );
            let polygon_indices = std::slice::from_raw_parts(indices_ptr, indices_count as usize);

            let mut features_count: ZenKitCAPI_sys::ZkSize = 0;
            let features_ptr = ZenKitCAPI_sys::ZkPolygon_getFeatureIndices(
                polygon,
                mesh,
                &mut features_count as *mut _,
            );
            let polygon_features =
                std::slice::from_raw_parts(features_ptr, features_count as usize);

            if polygon_indices.len() != 3 {
                println!(
                    "Skip polygon({polygon_index}) it has {} vertices and we support only polygons with 3 vertices, features_count({features_count})",
                    polygon_indices.len()
                );
                continue;
            }
            assert!(polygon_features.len() == polygon_indices.len());

            // for feature_index in polygon_features {
            //     let feature = ZenKitCAPI_sys::ZkMesh_getVertex(mesh, u64::from(*feature_index));
            //     // uvs[] = Vec2::from([feature.texture.x, feature.texture.y]););
            //     // normals.push(Vec3 {
            //     //     x: feature.normal.x,
            //     //     y: feature.normal.y,
            //     //     z: feature.normal.z,
            //     // });
            // }

            let index0 = 2;
            let index1 = 1;
            let index2 = 0;

            let index0 = 0;
            let index1 = 1;
            let index2 = 2;

            let material_index = ZenKitCAPI_sys::ZkPolygon_getMaterialIndex(polygon);
            let material = ZenKitCAPI_sys::ZkMesh_getMaterial(mesh, u64::from(material_index));
            let material_color = ZenKitCAPI_sys::ZkMaterial_getColor(material);
            let material_color = Vec4::from_array([
                material_color.r as f32 / 255.0,
                material_color.g as f32 / 255.0,
                material_color.b as f32 / 255.0,
                material_color.a as f32 / 255.0,
            ]);

            let feature0 =
                ZenKitCAPI_sys::ZkMesh_getVertex(mesh, u64::from(polygon_features[index0]));
            let feature1 =
                ZenKitCAPI_sys::ZkMesh_getVertex(mesh, u64::from(polygon_features[index1]));
            let feature2 =
                ZenKitCAPI_sys::ZkMesh_getVertex(mesh, u64::from(polygon_features[index2]));

            uvs.push(ZkVec2f_to_Vec2(feature0.texture));
            uvs.push(ZkVec2f_to_Vec2(feature1.texture));
            uvs.push(ZkVec2f_to_Vec2(feature2.texture));

            normals.push(ZkVec3f_to_Vec3(feature0.normal));
            normals.push(ZkVec3f_to_Vec3(feature1.normal));
            normals.push(ZkVec3f_to_Vec3(feature2.normal));

            colors.push(material_color);
            colors.push(material_color);
            colors.push(material_color);

            vertices.push(get_wolrld_pos(ZenKitCAPI_sys::ZkMesh_getPosition(
                mesh,
                u64::from(polygon_indices[index0]),
            )));
            vertices.push(get_wolrld_pos(ZenKitCAPI_sys::ZkMesh_getPosition(
                mesh,
                u64::from(polygon_indices[index1]),
            )));
            vertices.push(get_wolrld_pos(ZenKitCAPI_sys::ZkMesh_getPosition(
                mesh,
                u64::from(polygon_indices[index2]),
            )));

            indices.push(indices.len() as u32);
            indices.push(indices.len() as u32);
            indices.push(indices.len() as u32);
        }
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vertices)
    .with_inserted_indices(Indices::U32(indices))
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    // .with_computed_area_weighted_normals()
}

fn spawn_world(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    asset_server: Res<AssetServer>,
) {
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let floor = meshes.add(Plane3d::new(Vec3::Y, Vec2::splat(10.0)));
    let sphere = meshes.add(Sphere::new(0.5));
    let wall = meshes.add(Cuboid::new(0.2, 4.0, 3.0));

    let blue_material = materials.add(Color::from(tailwind::BLUE_700));
    let red_material = materials.add(Color::from(tailwind::RED_950));
    let white_material = materials.add(Color::WHITE);

    let mesh = create_gothic_world_mesh();
    let mesh_handle = meshes.add(mesh);
    let mesh_material = materials.add(Color::WHITE);
    commands.spawn((
        Mesh3d(mesh_handle.clone()),
        MeshMaterial3d(mesh_material.clone()),
    ));

    // let mesh = create_my_mesh();
    // let mesh_handle = meshes.add(mesh);
    // commands.spawn((
    //     Mesh3d(mesh_handle.clone()),
    //     MeshMaterial3d(red_material.clone()),
    // ));

    // Top side of floor
    // commands.spawn((
    //     Mesh3d(floor.clone()),
    //     MeshMaterial3d(white_material.clone()),
    // ));
    // // Under side of floor
    // commands.spawn((
    //     Mesh3d(floor.clone()),
    //     MeshMaterial3d(white_material.clone()),
    //     Transform::from_xyz(0.0, -0.01, 0.0).with_rotation(Quat::from_rotation_x(PI)),
    // ));
    // // Blue sphere
    // commands.spawn((
    //     Mesh3d(sphere.clone()),
    //     MeshMaterial3d(blue_material.clone()),
    //     Transform::from_xyz(3.0, 1.5, 0.0),
    // ));
    // // Tall wall
    // commands.spawn((
    //     Mesh3d(wall.clone()),
    //     MeshMaterial3d(white_material.clone()),
    //     Transform::from_xyz(-3.0, 2.0, 0.0),
    // ));
    // // Cube behind wall
    // commands.spawn((
    //     Mesh3d(cube.clone()),
    //     MeshMaterial3d(blue_material.clone()),
    //     Transform::from_xyz(-4.2, 0.5, 0.0),
    // ));
    // // Hidden cube under floor
    // commands.spawn((
    //     Mesh3d(cube.clone()),
    //     MeshMaterial3d(red_material.clone()),
    //     Transform {
    //         translation: Vec3::new(3.0, -2.0, 0.0),
    //         rotation: Quat::from_euler(EulerRot::YXZEx, FRAC_PI_4, FRAC_PI_4, 0.0),
    //         ..default()
    //     },
    // ));
    commands.spawn(
        // (
        SceneRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset("FlightHelmet.gltf"))),
        // Transform::from_xyz(0.0, 0.0, 2.0),
        // )
    );
}
