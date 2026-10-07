use crate::{misc::VfsOverwriteBehavior, vfs::Vfs};
use std::{env::home_dir, path::PathBuf};

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

pub fn worlds_dir() -> String {
    gothic2_dir() + "/Data/Worlds.vdf"
}

pub fn textures_dir() -> String {
    gothic2_dir() + "/Data/Textures.vdf"
}

pub fn mounted_vfs() -> Vfs {
    let vfs = Vfs::new();
    vfs.mount_disk_host(&worlds_dir(), VfsOverwriteBehavior::ALL);
    vfs.mount_disk_host(&textures_dir(), VfsOverwriteBehavior::ALL);
    vfs
}
