use bevy::prelude::*;
use zen_kit_rs::vfs::VfsNode;

// zenkit cords have mirrored X compared to bevy
pub const MIRROR_X: bool = true;

pub fn gothic2_dir() -> String {
    let dir =
        std::env::var("GOTHIC2_DIR").expect("GOTHIC_DIR2 environment varialdle has to be set");
    if let Some(dir_no_prefix) = dir.strip_suffix("/") {
        return dir_no_prefix.to_string();
    }
    dir
}

pub fn get_world_transform(gothic_mat: Mat4) -> Transform {
    let tr = Transform::from_matrix(gothic_mat);
    let pos = get_world_pos(tr.translation);
    let rot = get_world_rot(Mat3::from_quat(tr.rotation));
    Transform::from_translation(pos).with_rotation(rot)
}

pub fn get_world_pos(mut gothic_pos: Vec3) -> Vec3 {
    // X cords are mirrored
    if MIRROR_X {
        gothic_pos.x = -gothic_pos.x;
    }
    // World units are different
    gothic_pos / 100.0
}
pub fn get_world_rot(rot_mat: Mat3) -> Quat {
    let rot_euler = rot_mat.to_euler(EulerRot::XYZ);
    let mut quat = Quat::from_euler(EulerRot::XYZ, rot_euler.0, rot_euler.1, rot_euler.2);

    if MIRROR_X {
        // Do to X cords beeing mirrored we also have to modify rotation
        quat.y = -quat.y;
        quat.z = -quat.z;
    }
    quat
}

pub fn print_nodes(node: &VfsNode, level: u8) {
    let name = node.name();
    for _i in 0..level {
        print!(" ");
    }
    println!("{name}");

    let children = node.enumerate_children();
    for child in children {
        print_nodes(&child, level + 1);
    }
}

#[derive(Debug, Default)]
pub struct MeshData {
    pub indices: Vec<u32>,
    pub vertices: Vec<Vec3>,
    pub uvs: Vec<Vec2>,
    pub normals: Vec<Vec3>,
    pub colors: Vec<Vec4>,
    pub material: StandardMaterial,
}

#[derive(Debug, Clone)]
pub struct LoadedMeshData {
    pub texture: String,
    pub material: StandardMaterial,
    pub mesh: Mesh,
    pub transform: Transform,
    pub head_transform: Option<Transform>,
    pub name: String,
}

#[derive(Debug)]
pub struct MeshInstance {
    pub mesh_path: String,
    pub pos: Vec3,
    pub rot: Quat,
    pub is_colider: bool,
    pub texture_override: Option<String>,
}
#[derive(Debug)]
pub struct LightInstance {
    pub pos: Vec3,
    pub rot: Quat,
}
