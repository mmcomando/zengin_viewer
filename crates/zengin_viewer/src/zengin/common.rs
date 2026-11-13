use std::{
    collections::{HashMap, HashSet},
    env::home_dir,
    path::PathBuf,
};

use bevy::prelude::*;
use bevy::shape::Aabb3d;
use zen_kit_rs::vfs::VfsNode;

use crate::zengin::script::symbol_id::InstanceId;
use crate::zengin::visual::model::ZenGinModel;
use crate::zengin::waypoint_graph::WaypointGraph;

/// zenkit cords have mirrored X compared to bevy
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

/// Path of the compiled game script (`GOTHIC.DAT`).
pub fn gothic_dat_path() -> PathBuf {
    PathBuf::from(gothic2_dir()).join("_work/Data/Scripts/_compiled/GOTHIC.DAT")
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
    let texture = short_tex.to_uppercase();
    // Texture references with and without the .TGA extension exist, compiled
    // textures always end in -C.TEX
    let texture = if texture.ends_with(".TGA") {
        texture.replace(".TGA", "-C.TEX")
    } else {
        format!("{texture}-C.TEX")
    };
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

/// Animation file name used to probe the anim archive, `<SCHEME>-<ANIM>.MAN`.
pub fn animation_file_name(scheme: &str, animation: &str) -> String {
    format!("{}-{}.MAN", scheme.to_uppercase(), animation.to_uppercase())
}

/// Picks the animation scheme to use: the overlay when the archive has that
/// animation for the overlay scheme, the base scheme otherwise.
pub fn choose_animation_scheme<'a>(
    base: &'a str,
    overlay: Option<&'a str>,
    animation: &str,
    files: &HashSet<String>,
) -> &'a str {
    if let Some(overlay) = overlay
        && files.contains(&animation_file_name(overlay, animation))
    {
        return overlay;
    }
    base
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
pub struct ZenGinInstance {
    pub tr: Transform,
    pub archetype: String,
    pub collides: bool,
}

/// Attachment point of an interactive mob (`ZS_POS*` node of its mesh).
#[allow(dead_code)] // consumed by the mob use action
#[derive(Debug, Default, Clone)]
pub struct MobPosition {
    /// Node name, e.g. `ZS_POS0_FRONT`.
    pub name: String,
    /// Node transform in the mob model space.
    pub transform: Transform,
}

/// Interactive mob vob (`oCMobInter` and its subtypes).
#[derive(Debug, Default)]
pub struct ZenGinMob {
    pub tr: Transform,
    /// Mesh asset path, empty when the vob has no visual.
    pub archetype: String,
    pub collides: bool,
    /// Vob name, used by scripts and trigger targets.
    pub vob_name: String,
    /// `AI_UseMob` scheme derived from the visual stem.
    pub scheme: String,
    /// Number of interaction states (`stateNum`).
    pub state_num: i32,
    /// Script item instance required to use the mob, empty when none.
    pub use_with_item: String,
    /// Script function called when the mob changes state, empty when none.
    pub on_state_func: String,
    /// Script condition gating use of the mob, empty when none.
    pub condition_func: String,
    pub rewind: bool,
}

/// `AI_UseMob` scheme of a mob visual: the mesh stem up to the first `_` or
/// `.`, uppercased (`BENCH_NW_CITY_01.ASC` -> `BENCH`).
pub fn mob_scheme(visual_name: &str) -> String {
    visual_name
        .rsplit('/')
        .next()
        .unwrap_or(visual_name)
        .split(['_', '.'])
        .next()
        .unwrap_or_default()
        .to_uppercase()
}

#[derive(Debug, Default)]
pub struct ZenGinChest {
    pub tr: Transform,
    /// Archetype of the chest visual, `None` when the vob has no visual.
    pub archetype: String,
    pub collides: bool,
    /// Half-size of the selection collider, converted to world units.
    pub collider_half_extents: Vec3,
    pub name: String,
    pub items: Vec<(InstanceId, i32)>,
    pub locked: bool,
}

#[allow(dead_code)] // consumed by the music director step
#[derive(Debug, Clone)]
pub struct MusicZone {
    /// Raw vob name, e.g. `KHORINIS_KHO`.
    pub name: String,
    /// Zone bounds in Bevy world units.
    pub bounds: Aabb3d,
}

#[allow(dead_code)] // consumed by the music director step
impl MusicZone {
    /// Theme tag: the part of the vob name after the first `_`.
    ///
    /// `KHORINIS_KHO` -> `KHO`, which prefixes the `KHO_DAY_STD` music themes.
    pub fn tag(&self) -> &str {
        self.name
            .split_once('_')
            .map_or(self.name.as_str(), |(_, tag)| tag)
    }

    /// Same half-open point test as the original engine. [`Aabb3d`] only has
    /// volume-vs-volume containment, so the point check is manual. Degenerate
    /// boxes never contain a point.
    pub fn contains(&self, position: Vec3) -> bool {
        position.x >= self.bounds.min.x
            && position.x < self.bounds.max.x
            && position.y >= self.bounds.min.y
            && position.y < self.bounds.max.y
            && position.z >= self.bounds.min.z
            && position.z < self.bounds.max.z
    }
}

/// Music zones of the current world, replaced on every world spawn.
#[allow(dead_code)] // consumed by the music director step
#[derive(Resource, Debug, Default)]
pub struct MusicZones {
    pub zones: Vec<MusicZone>,
    pub default: Option<MusicZone>,
}

#[derive(Debug, Default)]
pub struct ZenGinWorldData {
    pub world_model: ZenGinModel,
    pub static_models: Vec<ZenGinInstance>,
    pub chests: Vec<ZenGinChest>,
    pub mobs: Vec<ZenGinMob>,
    pub spots: HashMap<String, Transform>,
    pub way_points: HashMap<String, Transform>,
    /// Names of all files in the compiled anim archive (uppercase)
    pub anim_files: HashSet<String>,
    /// Waypoint navigation graph built from the world way net
    pub waypoint_graph: WaypointGraph,
    pub music_zones: Vec<MusicZone>,
    pub default_music_zone: Option<MusicZone>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zone(name: &str, min: Vec3, max: Vec3) -> MusicZone {
        MusicZone {
            name: name.to_string(),
            bounds: Aabb3d::from_min_max(min, max),
        }
    }

    #[test]
    fn mob_scheme_is_the_stem_before_the_first_underscore_or_dot() {
        assert_eq!(mob_scheme("BENCH_NW_CITY_01.ASC"), "BENCH");
        assert_eq!(mob_scheme("BSANVIL_OC.MDS"), "BSANVIL");
        assert_eq!(mob_scheme("BEDHIGH_PC.ASC"), "BEDHIGH");
        assert_eq!(mob_scheme("chair"), "CHAIR");
        assert_eq!(mob_scheme(""), "");
    }

    #[test]
    fn tag_is_the_part_after_the_first_underscore() {
        let z = zone("KHORINIS_KHO", Vec3::ZERO, Vec3::ONE);
        assert_eq!(z.tag(), "KHO");

        let z = zone("DEFAULT_DEF", Vec3::ZERO, Vec3::ONE);
        assert_eq!(z.tag(), "DEF");

        let z = zone("PLAIN", Vec3::ZERO, Vec3::ONE);
        assert_eq!(z.tag(), "PLAIN");
    }

    #[test]
    fn contains_uses_half_open_bounds() {
        let z = zone("Z_Z", Vec3::ZERO, Vec3::splat(10.0));
        assert!(z.contains(Vec3::ZERO));
        assert!(z.contains(Vec3::splat(9.9)));
        assert!(!z.contains(Vec3::splat(10.0)));
        assert!(!z.contains(Vec3::splat(-0.1)));
    }

    #[test]
    fn degenerate_zone_contains_nothing() {
        let z = zone("Z_Z", Vec3::splat(5.0), Vec3::splat(5.0));
        assert!(!z.contains(Vec3::splat(5.0)));
    }
}
