use bevy::prelude::*;
use zen_kit_rs::vfs::VfsNode;

pub fn gothic2_dir() -> String {
    let dir =
        std::env::var("GOTHIC2_DIR").expect("GOTHIC_DIR2 environment varialdle has to be set");
    if let Some(dir_no_prefix) = dir.strip_suffix("/") {
        return dir_no_prefix.to_string();
    }
    return dir;
}

pub fn get_world_pos(mut gothic_pos: Vec3) -> Vec3 {
    // X cords are mirrored
    gothic_pos.x = -gothic_pos.x;
    // World units are different
    gothic_pos / 100.0
}
pub fn get_world_rot(rot_mat: Mat3) -> Quat {
    let mut rot_euler = rot_mat.to_euler(EulerRot::XYZ);
    // Do to X cords beeing mirrored we also have to modify rotation
    rot_euler.1 = -rot_euler.1;
    Quat::from_euler(EulerRot::XYZ, rot_euler.0, rot_euler.1, rot_euler.2)
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
}

#[derive(Debug)]
pub struct LoadedMeshData {
    pub texture: String,
    pub mesh: Mesh,
}

#[derive(Debug)]
pub struct MeshInstance {
    pub mesh_path: String,
    pub pos: Vec3,
    pub rot: Quat,
}
