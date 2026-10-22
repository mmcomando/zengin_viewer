use crate::conversions::*;
use crate::misc::Vec3;
use crate::misc::*;
use crate::vfs::Vfs;
use crate::vobs::virtual_object::Npc;
use crate::vobs::virtual_object::VirtualObject;
use crate::waynet::WayNet;
use crate::{mesh::Mesh, stream::Read};
use ZenKitCAPI_sys::*;
use std::ptr::NonNull;

#[derive(Debug)]
pub struct World {
    ptr: NonNull<ZkWorld>,
}

pub struct SpawnLocation<'a> {
    pub npc: Npc<'a>,
    pub position: Vec3,
    pub timer: f32,
}

impl World {
    pub unsafe fn from_raw_ptr(ptr: *mut ZkWorld) -> Option<Self> {
        NonNull::new(ptr).map(|ptr| Self { ptr })
    }

    pub fn load(reader: &Read) -> Option<Self> {
        let ptr = unsafe { ZkWorld_load(reader.as_raw()) };
        let ptr = NonNull::new(ptr)?;
        Some(Self { ptr })
    }

    pub fn load_path(path: &str) -> Option<Self> {
        let cstr = str_to_zk(path);
        unsafe {
            let raw = ZkWorld_loadPath(cstr.as_ptr());
            return Self::from_raw_ptr(raw);
        };
    }

    pub fn load_vfs(vfs: &Vfs, name: &str) -> Option<Self> {
        let cstr = str_to_zk(name);
        unsafe {
            let raw = ZkWorld_loadVfs(vfs.get_raw(), cstr.as_ptr());
            return Self::from_raw_ptr(raw);
        };
    }

    pub fn load_versioned(reader: &Read, version: GameVersion) -> Option<Self> {
        unsafe {
            let ptr = ZkWorld_loadVersioned(reader.as_raw(), version);
            return Self::from_raw_ptr(ptr);
        };
    }

    pub fn load_path_versioned(path: &str, version: GameVersion) -> Option<Self> {
        let cstr = str_to_zk(path);
        unsafe {
            let ptr = ZkWorld_loadPathVersioned(cstr.as_ptr(), version);
            return Self::from_raw_ptr(ptr);
        };
    }

    pub fn load_vfs_versioned(vfs: &Vfs, name: &str, version: GameVersion) -> Option<Self> {
        let cstr = str_to_zk(name);
        unsafe {
            let ptr = ZkWorld_loadVfsVersioned(vfs.get_raw(), cstr.as_ptr(), version);
            return Self::from_raw_ptr(ptr);
        };
    }

    pub fn npc_spawn_enabled(&self) -> bool {
        unsafe { ZkWorld_getNpcSpawnEnabled(self.ptr.as_ptr()).to_bool() }
    }

    pub fn npc_spawn_flags(&self) -> ::std::os::raw::c_int {
        unsafe { ZkWorld_getNpcSpawnFlags(self.ptr.as_ptr()) }
    }

    pub fn mesh(&self) -> Mesh<'_> {
        unsafe {
            let ptr = ZkWorld_getMesh(self.ptr.as_ptr());
            return Mesh::from_raw_ptr(ptr.cast_mut(), false)
                .expect("World should alwas contain world mesh");
        };
    }

    pub fn spawn_location_count(&self) -> u64 {
        unsafe { ZkWorld_getSpawnLocationCount(self.ptr.as_ptr()) }
    }

    pub fn spawn_location(&self, index: usize) -> SpawnLocation<'_> {
        unsafe {
            let loc = ZkWorld_getSpawnLocation(self.ptr.as_ptr(), index as u64);
            return SpawnLocation {
                npc: Npc::from_raw_ptr(loc.npc, false).unwrap(),
                position: loc.position.to_own(),
                timer: loc.timer,
            };
        };
    }

    pub fn root_object_count(&self) -> u64 {
        unsafe { ZkWorld_getRootObjectCount(self.ptr.as_ptr()) }
    }

    pub fn root_object(&self, index: u64) -> VirtualObject<'_> {
        unsafe {
            let ptr = ZkWorld_getRootObject(self.ptr.as_ptr(), index);
            return VirtualObject::from_raw_ptr(ptr.cast_mut(), false).unwrap();
        }
    }

    pub fn root_objects(&self) -> Vec<VirtualObject<'_>> {
        let count = self.root_object_count();
        (0..count).map(|i| self.root_object(i)).collect()
    }

    pub fn npc_count(&self) -> u64 {
        unsafe { ZkWorld_getNpcCount(self.ptr.as_ptr()) }
    }

    pub fn npc(&self, index: u64) -> Npc<'_> {
        unsafe {
            let ptr = ZkWorld_getNpc(self.ptr.as_ptr(), index);
            return Npc::from_raw_ptr(ptr, false).unwrap();
        }
    }

    pub fn npcs(&self) -> Vec<Npc<'_>> {
        let count = self.npc_count();
        (0..count).map(|i| self.npc(i)).collect()
    }

    pub fn way_net(&self) -> WayNet<'_> {
        unsafe {
            let ptr = ZkWorld_getWayNet(self.ptr.as_ptr().cast_const());
            return WayNet::from_raw_ptr(ptr.cast_mut(), false).unwrap();
        }
    }

    // pub fn ZkWorld_getWayNet(slf: *const ZkWorld) -> *const ZkWayNet;
}

impl Drop for World {
    fn drop(&mut self) {
        unsafe { ZkWorld_del(self.ptr.as_mut()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing_common::*;
    use crate::texture::Texture;

    #[test]
    fn read_world() {
        let _zenkit = zenkit_ffi();
        let vfs = mounted_vfs();

        let node = vfs
            .resolve_path("/_WORK/DATA/WORLDS/NEWWORLD/NEWWORLD.ZEN")
            .unwrap();
        let reader = node.open().unwrap();
        let world = World::load(&reader).unwrap();
        let mesh = world.mesh();
        let materials = mesh.materials();

        let node_texture = vfs.find_node("OW_OLDCAMP_WALL_17-C.TEX").unwrap();
        let node_texture_read = node_texture.open().unwrap();
        let texture = Texture::load(&node_texture_read).unwrap();
        assert_eq!(texture.mipmap_count(), 7);
        assert_eq!(materials[0].texture(), "NW_NATURE_STEIN_07.TGA");
    }
}

// pub fn ZkWorld_getBspTree(slf: *const ZkWorld) -> *const ZkBspTree;
// pub fn ZkWorld_getCutscenePlayer(slf: *const ZkWorld) -> *mut ZkCutscenePlayer;
// pub fn ZkWorld_getSkyController(slf: *const ZkWorld) -> *mut ZkSkyController;

// pub fn ZkWorld_save(slf: *mut ZkWorld, write: *mut ZkWrite, version: ZkGameVersion);
// pub fn ZkWorld_addRootObject(slf: *mut ZkWorld, val: *mut ZkVirtualObject);
// pub fn ZkWorld_clearRootObjects(slf: *mut ZkWorld);
// pub fn ZkWorld_setNpcSpawnEnabled(slf: *mut ZkWorld, val: ZkBool);
// pub fn ZkWorld_setNpcSpawnFlags(slf: *mut ZkWorld, val: ::std::os::raw::c_int);
// pub fn ZkWorld_clearNpcs(slf: *mut ZkWorld);
// pub fn ZkWorld_removeNpc(slf: *mut ZkWorld, i: ZkSize);
// pub fn ZkWorld_addNpc(slf: *mut ZkWorld, npc: *mut ZkNpc);
// pub fn ZkWorld_clearSpawnLocations(slf: *mut ZkWorld);
// pub fn ZkWorld_addSpawnLocation(slf: *mut ZkWorld, val: ZkSpawnLocation);
// pub fn ZkWorld_removeSpawnLoaction(slf: *mut ZkWorld, i: ZkSize);
// pub fn ZkWorld_setCutscenePlayer(slf: *mut ZkWorld, val: *mut ZkCutscenePlayer);
// pub fn ZkWorld_setSkyController(slf: *mut ZkWorld, val: *mut ZkSkyController);
