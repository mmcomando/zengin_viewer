//! Helpers resolving zengin asset paths against the vfs: mesh names, item and
//! npc model paths and stand animations.

use bevy::log::warn;
use bevy::prelude::*;
use zen_kit_rs::vfs::Vfs;

use crate::zengin::common::to_asset_path;
use crate::zengin::loaders::vdf_reader::ZenginVfs;
use crate::zengin::script::component::ScriptResource;
use crate::zengin::script::symbol_id::InstanceId;

pub fn normalize_mesh_name(name: &str) -> String {
    name.to_uppercase()
        .replace(".3DS", "")
        .replace(".MMS", "")
        .replace(".MDS", "")
        .replace(".ASC", "")
}

pub(crate) fn find_mesh_path(vfs: &Vfs, name: &str) -> Option<String> {
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
            .map(|instance| instance.chosen_visual().to_string())
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
pub(crate) fn resolve_mesh_path_quiet(vfs: &Vfs, name: &str) -> Option<String> {
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
