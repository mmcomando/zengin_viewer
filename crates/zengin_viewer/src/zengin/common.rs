use std::{
    collections::{BTreeMap, HashMap, HashSet},
    env::home_dir,
    path::PathBuf,
};

use bevy::prelude::*;
use bevy::shape::Aabb3d;
use zen_kit_rs::vfs::VfsNode;

use crate::zengin::waypoint_graph::WaypointGraph;
use crate::zengin::world_spawn::VobPrototype;

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

/// Sight distance when the npc has no `senses_range`, in meters:
/// `PERC_DIST_ACTIVE_MAX` is 2000 Gothic units and the viewer uses 100 Gothic
/// units per meter.
pub const DEFAULT_SIGHT_RANGE_METERS: f32 = 20.0;

/// Radius within which an npc may pick or use a mob of a scheme, in world units
/// (meters). `OpenGothic` `availableMob` uses 1000 engine units.
pub const MOB_SEARCH_RADIUS: f32 = 10.0;

/// Distance within which an npc counts as using a mob or standing on a
/// freepoint, in world units (meters).
pub const MAX_AI_USE_DISTANCE: f32 = 1.5;

/// Radius within which an npc may pick a freepoint.
pub const FREEPOINT_SEARCH_RADIUS: f32 = 8.2;

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

/// Offset from `from` to `to` with the vertical component dropped.
pub fn horizontal_offset(from: Vec3, to: Vec3) -> Vec3 {
    let offset = to - from;
    Vec3::new(offset.x, 0.0, offset.z)
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

/// Attachment point of an interactive mob (`ZS_POS*` node of its mesh).
#[allow(dead_code)] // consumed by the mob use action
#[derive(Debug, Default, Clone, Reflect)]
pub struct MobPosition {
    /// Node name, e.g. `ZS_POS0_FRONT`.
    pub name: String,
    /// Node transform in the mob model space.
    pub transform: Transform,
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

/// Schemes of switch-like mobs: levers, buttons, wheels, doors. Using one
/// toggles its state and ends the player's use automatically.
pub fn is_switch_mob_scheme(scheme: &str) -> bool {
    ["DOOR", "LEVER", "TOUCHPLATE", "TURNSWITCH", "VWHEEL"]
        .iter()
        .any(|name| scheme.eq_ignore_ascii_case(name))
}

/// File stem of a mob visual, the `<STEM>` of its `<STEM>-S_S<n>.MAN`
/// animations (`BSFIRE_OC.MDM` -> `BSFIRE_OC`).
pub fn mob_visual_stem(visual_name: &str) -> String {
    let file = visual_name.rsplit('/').next().unwrap_or(visual_name);
    let stem = file.rsplit_once('.').map_or(file, |(stem, _)| stem);
    stem.to_uppercase()
}

/// Half-open point-in-box test. [`Aabb3d`] only has volume-vs-volume
/// containment, so zone and trigger areas need this helper.
pub(crate) trait Aabb3dExt {
    fn contains_point(&self, point: Vec3) -> bool;
}

impl Aabb3dExt for Aabb3d {
    fn contains_point(&self, point: Vec3) -> bool {
        point.x >= self.min.x
            && point.x < self.max.x
            && point.y >= self.min.y
            && point.y < self.max.y
            && point.z >= self.min.z
            && point.z < self.max.z
    }
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

    /// Same half-open point test as the original engine.
    pub fn contains(&self, position: Vec3) -> bool {
        self.bounds.contains_point(position)
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
    pub spots: HashMap<String, Transform>,
    pub way_points: HashMap<String, Transform>,
    /// Waypoint navigation graph built from the world way net
    pub waypoint_graph: WaypointGraph,
    pub music_zones: Vec<MusicZone>,
    pub default_music_zone: Option<MusicZone>,
    /// `zCTriggerWorldStart` vobs of the world, fired once it is in game.
    pub world_start_triggers: Vec<WorldStartTrigger>,
    /// `oCTriggerChangeLevel` areas of the world.
    pub change_level_triggers: Vec<ChangeLevelTrigger>,
    /// Pre-resolved spawn data of every spawnable vob, keyed by archive id.
    pub prototypes: BTreeMap<u32, VobPrototype>,
}

/// Area that loads another world when the player enters it
/// (`oCTriggerChangeLevel`).
#[derive(Debug, Clone)]
pub struct ChangeLevelTrigger {
    /// Raw vob name, empty for unnamed triggers.
    pub name: String,
    /// Target world path without the `.ZEN` extension (`OLDWORLD/OLDWORLD`).
    pub level: String,
    /// Vob in the target world the player is placed at.
    pub start_vob: String,
    /// Trigger area in Bevy world units.
    pub bounds: Aabb3d,
    pub start_enabled: bool,
    pub respond_to_pc: bool,
}

impl ChangeLevelTrigger {
    /// Same half-open point test as the other zones. Degenerate boxes never
    /// contain a point.
    pub fn contains(&self, position: Vec3) -> bool {
        self.start_enabled && self.respond_to_pc && self.bounds.contains_point(position)
    }
}

/// Vob notified when the world starts (`zCTriggerWorldStart`).
#[derive(Debug, Clone)]
pub struct WorldStartTrigger {
    /// Name of the vob the trigger event is addressed to.
    pub target: String,
    /// `fireOnlyFirstTime`: ignored when a saved world is restored.
    pub fire_only_first_time: bool,
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
    fn face_direction_quat_faces_the_given_direction() {
        let directions = [
            Vec3::Z,
            Vec3::NEG_Z,
            Vec3::X,
            Vec3::NEG_X,
            Vec3::new(1.0, 0.0, 1.0).normalize(),
            Vec3::new(-3.0, 0.0, 2.0).normalize(),
        ];
        for direction in directions {
            let facing = face_direction_quat(direction) * Vec3::Z;
            assert!(
                facing.abs_diff_eq(direction, 1e-4),
                "facing {facing:?} does not match {direction:?}"
            );
        }
    }

    #[test]
    fn mover_keyframe_rotation_matches_the_vob_rest_pose() {
        use crate::zengin::loaders::ZenginQuatToGlam;

        // NW_MISC_WINDMILL_ROTOR_01: the vob rotation and keyframe 0 describe
        // the same orientation. The keyframe quaternion is transposed relative
        // to the vob matrix, hence the conjugation.
        let vob = Mat3::from_cols(
            Vec3::new(0.017, -0.105, 0.994),
            Vec3::new(0.002, 0.995, 0.105),
            Vec3::new(-1.000, 0.000, 0.017),
        );
        let keyframe = zen_kit_rs::misc::Quat::from_xyzw(-0.037, 0.700, 0.037, 0.712);

        let from_vob = get_world_rot(vob);
        let from_keyframe = get_world_quat(keyframe.to_glam().conjugate());
        assert!(
            from_vob.dot(from_keyframe).abs() > 0.9995,
            "keyframe rotation must match the vob rest pose: {from_vob:?} vs {from_keyframe:?}"
        );
    }

    #[test]
    fn change_level_area_is_half_open_and_respects_flags() {
        let mut trigger = ChangeLevelTrigger {
            name: "NW_2_OW".to_string(),
            level: "OLDWORLD/OLDWORLD".to_string(),
            start_vob: "START_OW".to_string(),
            bounds: Aabb3d::from_min_max(Vec3::ZERO, Vec3::ONE),
            start_enabled: true,
            respond_to_pc: true,
        };
        assert!(trigger.contains(Vec3::new(0.5, 0.5, 0.5)));
        assert!(
            !trigger.contains(Vec3::new(1.0, 0.5, 0.5)),
            "max is exclusive"
        );
        assert!(!trigger.contains(Vec3::new(0.5, -0.1, 0.5)));

        trigger.respond_to_pc = false;
        assert!(!trigger.contains(Vec3::new(0.5, 0.5, 0.5)));
        trigger.respond_to_pc = true;
        trigger.start_enabled = false;
        assert!(!trigger.contains(Vec3::new(0.5, 0.5, 0.5)));
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
    fn switch_mob_schemes_are_recognised_case_insensitively() {
        for scheme in ["DOOR", "lever", "ToUcHpLaTe", "turnsWITCH", "vwheel"] {
            assert!(is_switch_mob_scheme(scheme), "{scheme} is a switch");
        }
        for scheme in ["BENCH", "CHAIR", "BSANVIL", ""] {
            assert!(!is_switch_mob_scheme(scheme), "{scheme} is not a switch");
        }
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
