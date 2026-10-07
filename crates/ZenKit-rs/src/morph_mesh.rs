use crate::conversions::*;
use crate::misc::Vec3;
use crate::mrs_mesh::MrsMesh;
use crate::stream::Read;
use crate::vfs::Vfs;
use ZenKitCAPI_sys::*;
use std::marker::PhantomData;
use std::ptr::NonNull;

#[derive(Debug)]
pub struct MorphMesh<'a> {
    ptr: NonNull<ZkMorphMesh>,
    _marker: PhantomData<&'a ZkMorphMesh>,
    owned: bool,
}

impl MorphMesh<'_> {
    pub unsafe fn from_raw_ptr(ptr: *mut ZkMorphMesh, owned: bool) -> Option<Self> {
        NonNull::new(ptr).map(|ptr| Self {
            ptr,
            owned,
            _marker: PhantomData,
        })
    }

    pub fn load(read: &Read) -> Option<Self> {
        unsafe {
            let ptr = ZkMorphMesh_load(read.as_raw());
            return Self::from_raw_ptr(ptr, true);
        };
    }

    pub fn load_path(path: &str) -> Option<Self> {
        let cstr = str_to_zk(path);
        unsafe {
            let ptr = ZkMorphMesh_loadPath(cstr.as_ptr());
            return Self::from_raw_ptr(ptr, true);
        };
    }

    pub fn load_vfs(vfs: &Vfs, name: &str) -> Option<Self> {
        let cstr = str_to_zk(name);
        unsafe {
            let ptr = ZkMorphMesh_loadVfs(vfs.get_raw(), cstr.as_ptr());
            return Self::from_raw_ptr(ptr, true);
        };
    }

    pub fn name(&self) -> String {
        unsafe {
            let ptr = ZkMorphMesh_getName(self.ptr.as_ptr());
            ptr.to_string()
        }
    }

    pub fn mesh(&self) -> MrsMesh<'_> {
        unsafe {
            let raw = ZkMorphMesh_getMesh(self.ptr.as_ptr());
            return MrsMesh::from_raw_ptr(raw.cast_mut(), false).unwrap();
        };
    }

    pub fn morph_position_count(&self) -> u64 {
        unsafe { ZkMorphMesh_getMorphPositionCount(self.ptr.as_ptr()) }
    }

    pub fn morph_position(&self, index: u64) -> Vec3 {
        unsafe { ZkMorphMesh_getMorphPosition(self.ptr.as_ptr(), index).to_own() }
    }

    pub fn morph_positions(&self) -> Vec<Vec3> {
        let count = self.morph_position_count();
        (0..count).map(|i| self.morph_position(i)).collect()
    }
}

impl Drop for MorphMesh<'_> {
    fn drop(&mut self) {
        if self.owned {
            unsafe { ZkMorphMesh_del(self.ptr.as_ptr()) };
        }
    }
}

// pub fn ZkMorphMesh_getAnimationCount(slf: *const ZkMorphMesh) -> ZkSize;
// pub fn ZkMorphMesh_getAnimation(slf: *const ZkMorphMesh, i: ZkSize) -> *const ZkMorphAnimation;
// pub fn ZkMorphMesh_enumerateAnimations(
// pub fn ZkMorphMesh_getSourceCount(slf: *const ZkMorphMesh) -> ZkSize;
// pub fn ZkMorphMesh_getSource(slf: *const ZkMorphMesh, i: ZkSize) -> *const ZkMorphSource;
