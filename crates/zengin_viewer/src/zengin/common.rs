use std::{collections::HashMap, env::home_dir, path::PathBuf};

use bevy::prelude::*;
use zen_kit_rs::vfs::VfsNode;

use crate::zengin::script::symbol_id::InstanceId;

// zenkit cords have mirrored X compared to bevy
pub const MIRROR_X: bool = true;

/// Zentg (Gothic) game units per Bevy world unit (meter). World positions are
/// scaled down by this when loaded.
pub const ZENGIN_UNITS_PER_WORLD_UNIT: f32 = 100.0;

/// Converts a Bevy length (world units/meters) to engine (zengin) game units.
pub fn to_zengin_units(world_units: f32) -> f32 {
    world_units * ZENGIN_UNITS_PER_WORLD_UNIT
}

/// Converts an engine (zengin) length to Bevy world units (meters).
pub fn to_world_units(zengin_units: f32) -> f32 {
    zengin_units / ZENGIN_UNITS_PER_WORLD_UNIT
}

pub fn gothic2_dir() -> String {
    if let Ok(dir) = std::env::var("GOTHIC2_DIR") {
        let path = PathBuf::from(dir);
        return path.to_str().unwrap().to_string();
    }

    // Try to detect gothic2 location

    if let Some(home) = home_dir() {
        let linux_steam = home.join(".local/share/Steam/steamapps/common/Gothic II");
        if linux_steam.is_dir() {
            return linux_steam.to_str().unwrap().to_string();
        }
    }

    panic!(
        "Gothic2 location not found automatically. Set GOTHIC2_DIR environment variable to explicitly state game directory."
    );
}

pub fn to_asset_path(zengin_asset_path: &str) -> String {
    format!("zengin:/{}", zengin_asset_path)
}

pub fn get_world_transform(zengin_mat: Mat4) -> Transform {
    let tr = Transform::from_matrix(zengin_mat);
    let pos = get_world_pos(tr.translation);
    let rot = get_world_quat(tr.rotation);
    Transform::from_translation(pos).with_rotation(rot)
}

pub fn get_world_pos(mut zengin_pos: Vec3) -> Vec3 {
    // X cords are mirrored
    if MIRROR_X {
        zengin_pos.x = -zengin_pos.x;
    }
    // World units are different
    zengin_pos.map(to_world_units)
}

pub fn get_world_quat(mut quat: Quat) -> Quat {
    if MIRROR_X {
        // Do to X cords being mirrored we also have to modify rotation
        quat.y = -quat.y;
        quat.z = -quat.z;
    }
    quat
}

pub fn get_world_rot(rot_mat: Mat3) -> Quat {
    let quat = Quat::from_mat3(&rot_mat);
    get_world_quat(quat)
}

/// Rotation facing the given 2D direction (gothic X, Z), following the Gothic
/// NPC angle convention facing angle is "90deg + atan2(z, x)" ("-90deg" for a zero direction) and the model is
/// rotated by "180deg - angle" around Y.
pub fn get_world_dir_quat(dir_2d: Vec2) -> Quat {
    let yaw = dir_2d.to_angle() - std::f32::consts::FRAC_PI_2;
    // `get_world_quat` negates the Y rotation, hence the negated yaw passed in.
    get_world_quat(Quat::from_rotation_y(-yaw))
}

/// Rotation making an npc model face the given Bevy-space horizontal direction
/// (from the npc toward its target). Uses the same convention as waypoint
/// placement, so it only produces a yaw.
pub fn face_direction_quat(direction: Vec3) -> Quat {
    let x = if MIRROR_X { -direction.x } else { direction.x };
    get_world_dir_quat(Vec2::new(x, direction.z))
}

pub fn get_full_texture_path(short_tex: &str) -> String {
    let texture = short_tex.to_uppercase().replace(".TGA", "-C.TEX");
    format!("zengin://_WORK/DATA/TEXTURES/_COMPILED/{texture}")
}

/// Skeletal animations are stored per visual scheme, the file prefix is the
/// scheme (visual `.mds`) name: `HUMANS-S_RUN.MAN`, `WOLF-S_FISTRUN.MAN`.
pub fn get_animation_path(scheme: &str, animation: &str) -> String {
    format!(
        "zengin://_WORK/DATA/ANIMS/_COMPILED/{}-{}.MAN",
        scheme.to_uppercase(),
        animation.to_uppercase()
    )
}

/// Speech wav files are named after the Ou.bin block of the dialog line
pub fn get_dialog_sound_path(block_name: &str) -> String {
    let file = block_name.to_uppercase();
    let file = format!("{file}.WAV");
    format!("zengin://_WORK/DATA/SOUND/SPEECH/{file}")
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

    pub weights: Vec<[f32; 4]>,
    pub bone_indices: Vec<[u16; 4]>,

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
    pub is_skinned: bool,
}

#[derive(Debug)]
pub struct LightInstance {
    pub pos: Vec3,
    pub rot: Quat,
}

#[derive(Debug, Default, Clone, Asset, TypePath)]
pub struct ZenGinModel {
    pub sub_meshes: Vec<ZenGinSubMesh>,
    pub final_tr: Vec<Transform>,
    pub node_names: Vec<String>,
    pub nodes: Vec<Transform>,
    pub inverse_bindposes: Vec<Mat4>,
    pub parents: Vec<i16>,
    pub animation_bone_index: Vec<usize>,
}

#[derive(Debug, Default)]
pub struct ZenGinItem {
    pub tr: Transform,
    pub model: String,
}
#[derive(Debug)]
pub struct ZenGinNpc {
    pub npc_intance_id: InstanceId,
    pub head_model: Option<String>,
    pub head_texture: Option<String>,
    pub body_tr: Transform,
    pub body_model: String,
    pub body_texture: Option<String>,
    pub armor_model: Option<String>,
    pub hierarchy: Option<String>,
    /// Animation scheme (visual `.mds` stem) used as animation file prefix
    pub animation_scheme: Option<String>,
    /// Standing body state animation of the scheme
    pub stand_animation: Option<String>,
    /// Body fatness (`Mdl_SetModelFatness`)
    pub model_fatness: Option<f32>,
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
    /// Item symbol id to weapon mesh asset path (`EquipItem`)
    pub item_mesh_paths: HashMap<i32, String>,
}
