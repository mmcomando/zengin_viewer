use std::collections::HashMap;

use bevy::prelude::*;
use zen_kit_rs::vfs::VfsNode;

// zenkit cords have mirrored X compared to bevy
pub const MIRROR_X: bool = true;

pub fn gothic2_dir() -> String {
    let dir = std::env::var("GOTHIC2_DIR").expect("GOTHIC2_DIR environment variable has to be set");
    if let Some(dir_no_prefix) = dir.strip_suffix("/") {
        return dir_no_prefix.to_string();
    }
    dir
}

pub fn to_asset_path(zengin_asset_path: &str) -> String {
    format!("zengin:/{}", zengin_asset_path)
}

pub fn get_world_transform(zengin_mat: Mat4) -> Transform {
    let tr = Transform::from_matrix(zengin_mat);
    let pos = get_world_pos(tr.translation);
    let rot = get_world_rot(Mat3::from_quat(tr.rotation));
    Transform::from_translation(pos).with_rotation(rot)
}

pub fn get_world_pos(mut zengin_pos: Vec3) -> Vec3 {
    // X cords are mirrored
    if MIRROR_X {
        zengin_pos.x = -zengin_pos.x;
    }
    // World units are different
    zengin_pos / 100.0
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

pub fn get_full_texture_path(short_tex: &str) -> String {
    let texture = short_tex.to_uppercase().replace(".TGA", "-C.TEX");
    format!("zengin://_WORK/DATA/TEXTURES/_COMPILED/{texture}")
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
    pub collides: bool,
}

#[derive(Debug, Clone)]
pub struct ZenGinSubMesh {
    pub texture: String,
    pub material: StandardMaterial,
    pub mesh: Mesh,
    pub transform: Transform,
    // pub head_transform: Option<Transform>,
    pub name: String,
    pub collides: bool,
}

#[derive(Debug)]
pub struct LightInstance {
    pub pos: Vec3,
    pub rot: Quat,
}

#[derive(Debug, Default, Clone, Asset, TypePath)]
pub struct ZenGinModel {
    pub sub_meshes: Vec<ZenGinSubMesh>,
    pub nodes_tr: HashMap<String, Transform>,
}

#[derive(Debug, Default)]
pub struct ZenGinItem {
    pub tr: Transform,
    pub model: String,
}
#[derive(Debug, Default)]
pub struct ZenGinNpc {
    pub head_model: Option<String>,
    pub head_texture: Option<String>,
    pub body_tr: Transform,
    pub body_model: String,
    pub body_texture: Option<String>,
    pub armor_model: Option<String>,
}

#[derive(Debug, Default)]
pub struct ZenGinInstance {
    pub tr: Transform,
    pub archetype: String,
}

#[derive(Debug, Default)]
pub struct ZenGinWorldData {
    // pub tr: Transform,
    pub world_model: ZenGinModel,
    pub static_models: Vec<ZenGinInstance>,
    pub light_instances: Vec<LightInstance>,
    pub npcs: Vec<ZenGinNpc>,
    pub items: Vec<ZenGinItem>,
    pub spots: HashMap<String, Transform>,
    pub way_points: HashMap<String, Transform>,
}
