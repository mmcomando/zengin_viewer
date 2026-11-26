//! Helpers resolving zengin asset paths against the vfs: mesh names, item and
//! npc model paths and stand animations.

use std::collections::HashSet;

use bevy::log::warn;
use bevy::prelude::*;
use zen_kit_rs::vfs::Vfs;

use crate::common::{get_full_texture_path, to_asset_path};
use crate::loaders::vdf_reader::{ZenginVfs, get_zengin_asset_bytes};
use crate::script::component::ScriptResource;
use crate::script::symbol_id::InstanceId;

pub fn normalize_mesh_name(name: &str) -> String {
    name.to_uppercase()
        .replace(".3DS", "")
        .replace(".MMS", "")
        .replace(".MDS", "")
        .replace(".ASC", "")
}

pub fn find_mesh_path(vfs: &Vfs, name: &str) -> Option<String> {
    let name = normalize_mesh_name(name);

    let asset_paths: Vec<String> = vec![
        format!("/_WORK/DATA/ANIMS/_COMPILED/{}.MMB", name),
        format!("/_WORK/DATA/ANIMS/_COMPILED/{}.MDL", name),
        format!("/_WORK/DATA/ANIMS/_COMPILED/{}.MDM", name),
        format!("/_WORK/DATA/ANIMS/_COMPILED/{}.MDH", name),
        format!("/_WORK/DATA/MESHES/_COMPILED/{}.MDB", name),
        format!("/_WORK/DATA/MESHES/_COMPILED/{}.MRM", name),
        format!("/_WORK/DATA/MESHES/_COMPILED/{}.MSH", name),
    ];

    let Some(asset_path) = try_load_mesh(vfs, &asset_paths) else {
        warn!("Some meshes are not found, example({})", name);
        return None;
    };

    let asset_path = to_asset_path(&asset_path);
    return Some(asset_path);
}

/// Names of all files in the compiled anim archive, uppercased. The archive
/// contents are a property of the vfs, not of a loaded world, so this is
/// resolved once at startup.
pub fn animation_file_names(vfs: &ZenginVfs) -> HashSet<String> {
    let vfs = vfs.0.lock().unwrap();
    let Some(dir) = vfs.resolve_path("/_WORK/DATA/ANIMS/_COMPILED") else {
        warn!("Compiled anim archive directory not found, scripted animations cannot be resolved");
        return HashSet::new();
    };
    dir.enumerate_children()
        .into_iter()
        .map(|child| child.name().to_uppercase())
        .collect()
}

/// Standing body state animation of an animation scheme. `S_RUN` is the human
/// idle, most creatures only provide `S_FISTRUN`; probe the anims archive for
/// the one the scheme has.
/// Stand (walk) animation of an animation scheme, resolved against the shared
/// vfs, used when spawning npcs from spawn messages.
pub fn resolve_stand_animation(vfs: &ZenginVfs, scheme: &str) -> Option<String> {
    let vfs = vfs.0.lock().unwrap();
    ["S_RUN", "S_FISTRUN"]
        .into_iter()
        .find(|ani| {
            vfs.resolve_path(&format!("/_WORK/DATA/ANIMS/_COMPILED/{scheme}-{ani}.MAN"))
                .is_some()
        })
        .map(str::to_string)
}

/// Resolved asset path of a texture referenced by a script (for example
/// `chapter1.tga`). Tries the compiled-textures directory first, then the bare
/// compiled file name, mirroring the engine's texture lookup.
pub fn resolve_texture_asset(vfs: &ZenginVfs, short: &str) -> Option<String> {
    let candidates = [
        get_full_texture_path(short),
        format!("zengin://{}", compiled_texture_name(short)),
    ];
    candidates
        .iter()
        .find(|path| zengin_asset_exists(vfs, path))
        .cloned()
}

/// Resolved asset path of a global sound referenced by a script (for example
/// `chapter_01.wav`). Tries the global sound directory first, then the bare
/// file name.
pub fn resolve_sound_asset(vfs: &ZenginVfs, name: &str) -> Option<String> {
    let file = sound_file_name(name);
    let candidates = [
        format!("zengin://_WORK/DATA/SOUND/{file}"),
        format!("zengin://{file}"),
    ];
    candidates
        .iter()
        .find(|path| zengin_asset_exists(vfs, path))
        .cloned()
}

/// `<NAME>-C.TEX` of a texture short name, with or without its `.TGA` suffix.
fn compiled_texture_name(short: &str) -> String {
    let texture = short.to_uppercase();
    if texture.ends_with(".TGA") {
        texture.replace(".TGA", "-C.TEX")
    } else {
        format!("{texture}-C.TEX")
    }
}

/// Upper-cased `.WAV` file name of a sound short name.
fn sound_file_name(name: &str) -> String {
    let file = name.to_uppercase();
    if file.ends_with(".WAV") {
        file
    } else {
        format!("{file}.WAV")
    }
}

/// Whether the vfs contains the given `zengin://` asset path. Bare file names
/// fall back to the archive-wide name lookup of `get_zengin_asset_bytes`.
fn zengin_asset_exists(vfs: &ZenginVfs, asset_path: &str) -> bool {
    let relative = asset_path.strip_prefix("zengin://").unwrap_or(asset_path);
    let vfs = vfs
        .0
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    get_zengin_asset_bytes(&vfs, relative).is_some()
}

/// Resolved mesh asset path of an item instance, from the script item instances
/// and the shared vfs. Used at runtime, where the world load vfs is gone.
pub fn item_model_path(
    scripts: &ScriptResource,
    vfs: &ZenginVfs,
    item: InstanceId,
) -> Option<String> {
    let visual = {
        let script = scripts.lock();
        script
            .get_loaded_state()
            .items_instances
            .get(&item)
            .map(|instance| {
                instance
                    .read()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .chosen_visual()
                    .to_string()
            })
    }?;
    let vfs = vfs.0.lock().unwrap();
    resolve_mesh_path_quiet(&vfs, &visual)
}

/// Resolved mesh asset path of a named mesh (body/head/hierarchy/armor), used
/// when spawning npcs from spawn messages, where the world load vfs is gone.
pub fn mesh_model_path(vfs: &ZenginVfs, name: &str) -> Option<String> {
    let vfs = vfs.0.lock().unwrap();
    resolve_mesh_path_quiet(&vfs, name)
}

/// Resolved skinned body mesh path of an npc.
///
/// Prefers `.MDM`, which only stores skin weights and relies on the visual
/// `.MDH` hierarchy (`Mdl_SetVisual`, e.g. `SHEEP.MDH`). That is also the
/// hierarchy the `.MAN` node indices refer to. A `.MDL` embeds its own
/// hierarchy (with extra `*NUB` bones, e.g. 37 vs 31 nodes for the sheep), so
/// its node indices do not line up with the animation samples and the npc
/// animates the wrong bones.
pub fn mesh_body_path(vfs: &ZenginVfs, name: &str) -> Option<String> {
    let vfs = vfs.0.lock().unwrap();
    let name = normalize_mesh_name(name);
    let asset_paths: Vec<String> = vec![
        format!("/_WORK/DATA/ANIMS/_COMPILED/{name}.MDM"),
        format!("/_WORK/DATA/MESHES/_COMPILED/{name}.MDM"),
        format!("/_WORK/DATA/ANIMS/_COMPILED/{name}.MDL"),
        format!("/_WORK/DATA/MESHES/_COMPILED/{name}.MDL"),
    ];
    try_load_mesh(&vfs, &asset_paths)
        .map(|path| to_asset_path(&path))
        .or_else(|| resolve_mesh_path_quiet(&vfs, &name))
}

fn try_load_mesh(vfs: &Vfs, asset_paths: &[String]) -> Option<String> {
    for asset_path in asset_paths {
        if vfs.resolve_path(asset_path).is_some() {
            return Some(asset_path.clone());
        }
    }

    return None;
}

/// Like `find_mesh_path` but returns `None` silently, for the bulk item mesh
/// table where missing meshes are expected.
pub fn resolve_mesh_path_quiet(vfs: &Vfs, name: &str) -> Option<String> {
    let name = normalize_mesh_name(name);
    let asset_paths: Vec<String> = vec![
        format!("/_WORK/DATA/ANIMS/_COMPILED/{}.MMB", name),
        format!("/_WORK/DATA/ANIMS/_COMPILED/{}.MDL", name),
        format!("/_WORK/DATA/ANIMS/_COMPILED/{}.MDM", name),
        format!("/_WORK/DATA/ANIMS/_COMPILED/{}.MDH", name),
        format!("/_WORK/DATA/MESHES/_COMPILED/{}.MDB", name),
        format!("/_WORK/DATA/MESHES/_COMPILED/{}.MRM", name),
        format!("/_WORK/DATA/MESHES/_COMPILED/{}.MSH", name),
    ];
    try_load_mesh(vfs, &asset_paths).map(|path| to_asset_path(&path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loaders::vdf_reader::create_zengin_vfs;

    #[test]
    #[ignore = "requires a Gothic 2 install"]
    fn body_mesh_prefers_the_weight_only_mdm() {
        let vfs = ZenginVfs(create_zengin_vfs());
        // Both `HAMMEL_BODY.MDL` (own 37-bone hierarchy) and `.MDM` exist; the
        // animation hierarchy `SHEEP.MDH` has 31 bones, so the `.MDM` is the
        // only representation whose node indices match the `.MAN` files.
        assert_eq!(
            mesh_body_path(&vfs, "Hammel_Body").as_deref(),
            Some("zengin://_WORK/DATA/ANIMS/_COMPILED/HAMMEL_BODY.MDM")
        );
    }

    #[test]
    #[ignore = "requires a Gothic 2 install"]
    fn chapter_assets_resolve() {
        let vfs = ZenginVfs(create_zengin_vfs());
        assert_eq!(
            resolve_texture_asset(&vfs, "chapter1.tga").as_deref(),
            Some("zengin://_WORK/DATA/TEXTURES/_COMPILED/CHAPTER1-C.TEX")
        );
        // Global sounds are not under a fixed directory, the archive-wide name
        // lookup finds them.
        assert_eq!(
            resolve_sound_asset(&vfs, "chapter_01.wav").as_deref(),
            Some("zengin://CHAPTER_01.WAV")
        );
    }
}
