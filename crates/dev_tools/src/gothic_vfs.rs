//! Gothic 2 game directory and virtual file system access.

use std::env::home_dir;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use zen_kit_rs::misc::VfsOverwriteBehavior;
use zen_kit_rs::vfs::Vfs;

/// Gothic 2 installation directory, taken from `GOTHIC2_DIR` when set.
pub fn gothic2_dir() -> String {
    if let Ok(dir) = std::env::var("GOTHIC2_DIR") {
        let path = PathBuf::from(dir);
        return path.to_str().unwrap().to_string();
    }

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

/// The game archives mounted with the loose-file override the game itself uses.
pub fn create_zengin_vfs() -> Arc<Mutex<Vfs>> {
    let vfs_override = VfsOverwriteBehavior::ALL;
    let dir = gothic2_dir();

    let vfs = Vfs::new();
    vfs.mount_disk_host(&format!("{}/Data/Anims.vdf", dir), vfs_override);
    vfs.mount_disk_host(&format!("{}/Data/Anims_Addon.vdf", dir), vfs_override);
    vfs.mount_disk_host(&format!("{}/Data/Meshes.vdf", dir), vfs_override);
    vfs.mount_disk_host(&format!("{}/Data/Meshes_Addon.vdf", dir), vfs_override);
    vfs.mount_disk_host(&format!("{}/Data/Sounds.vdf", dir), vfs_override);
    vfs.mount_disk_host(&format!("{}/Data/Sounds_Addon.vdf", dir), vfs_override);
    vfs.mount_disk_host(&format!("{}/Data/Sounds_bird_01.vdf", dir), vfs_override);
    vfs.mount_disk_host(&format!("{}/Data/Speech1.vdf", dir), vfs_override);
    vfs.mount_disk_host(&format!("{}/Data/Speech2.vdf", dir), vfs_override);
    vfs.mount_disk_host(&format!("{}/Data/Speech_Addon.vdf", dir), vfs_override);
    vfs.mount_disk_host(
        &format!("{}/Data/Speech_English_Patch_Atari.vdf", dir),
        vfs_override,
    );
    vfs.mount_disk_host(
        &format!("{}/Data/Speech_heyou_citygde_engl.vdf", dir),
        vfs_override,
    );
    vfs.mount_disk_host(
        &format!("{}/Data/Speech_Parlan_engl.vdf", dir),
        vfs_override,
    );
    vfs.mount_disk_host(&format!("{}/Data/Scripts.vdf", dir), vfs_override);
    vfs.mount_disk_host(&format!("{}/Data/SystemPack.vdf", dir), vfs_override);
    vfs.mount_disk_host(&format!("{}/Data/Textures.vdf", dir), vfs_override);
    vfs.mount_disk_host(&format!("{}/Data/Textures_Addon.vdf", dir), vfs_override);
    vfs.mount_disk_host(
        &format!("{}/Data/Textures_Addon_Menu_English.vdf", dir),
        vfs_override,
    );
    vfs.mount_disk_host(
        &format!("{}/Data/Textures_Fonts_Apostroph.vdf", dir),
        vfs_override,
    );
    vfs.mount_disk_host(
        &format!("{}/Data/Textures_multilingual_Jowood.vdf", dir),
        vfs_override,
    );
    vfs.mount_disk_host(&format!("{}/Data/Worlds.vdf", dir), vfs_override);
    vfs.mount_disk_host(&format!("{}/Data/Worlds_Addon.vdf", dir), vfs_override);

    Arc::new(Mutex::new(vfs))
}

/// Bytes of a file in the archive. A bare file name falls back to an
/// archive-wide lookup, matching the loose-file override.
pub fn get_zengin_asset_bytes(vfs: &Vfs, path: &str) -> Option<Vec<u8>> {
    let relative = path.trim_start_matches('/');
    let resolved = vfs.resolve_path(&format!("/{relative}"));
    let node = match resolved {
        Some(node) => node,
        None if !relative.contains('/') => vfs.find_node(relative)?,
        None => return None,
    };
    let read_obj = node.open()?;

    Some(read_obj.bytes())
}
