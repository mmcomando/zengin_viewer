use crate::conversions::*;
use crate::misc::AnimationSample;
use crate::stream::Read;
use crate::vfs::Vfs;
use ZenKitCAPI_sys::*;
use std::marker::PhantomData;
use std::ptr::NonNull;

#[derive(Debug)]
pub struct ModelAnimation<'a> {
    ptr: NonNull<ZkModelAnimation>,
    _marker: PhantomData<&'a ZkModelAnimation>,
    owned: bool,
}

impl ModelAnimation<'_> {
    pub unsafe fn from_raw_ptr(ptr: *mut ZkModelAnimation, owned: bool) -> Option<Self> {
        NonNull::new(ptr).map(|ptr| Self {
            ptr,
            owned,
            _marker: PhantomData,
        })
    }

    pub fn load(read: &Read) -> Option<Self> {
        unsafe {
            let ptr = ZkModelAnimation_load(read.as_raw());
            return Self::from_raw_ptr(ptr, true);
        };
    }

    pub fn load_path(path: &str) -> Option<Self> {
        let cstr = str_to_zk(path);
        unsafe {
            let ptr = ZkModelAnimation_loadPath(cstr.as_ptr());
            return Self::from_raw_ptr(ptr, true);
        };
    }

    pub fn load_vfs(vfs: &Vfs, name: &str) -> Option<Self> {
        let cstr = str_to_zk(name);
        unsafe {
            let ptr = ZkModelAnimation_loadVfs(vfs.get_raw(), cstr.as_ptr());
            return Self::from_raw_ptr(ptr, true);
        };
    }

    pub fn name(&self) -> String {
        unsafe { ZkModelAnimation_getName(self.ptr.as_ptr()).to_string() }
    }
    pub fn next(&self) -> String {
        unsafe { ZkModelAnimation_getNext(self.ptr.as_ptr()).to_string() }
    }
    pub fn layer(&self) -> u32 {
        unsafe { ZkModelAnimation_getLayer(self.ptr.as_ptr()) }
    }
    pub fn frame_count(&self) -> u32 {
        unsafe { ZkModelAnimation_getFrameCount(self.ptr.as_ptr()) }
    }
    pub fn node_count(&self) -> u32 {
        unsafe { ZkModelAnimation_getNodeCount(self.ptr.as_ptr()) }
    }
    pub fn fps(&self) -> f32 {
        unsafe { ZkModelAnimation_getFps(self.ptr.as_ptr()) }
    }
    pub fn fps_source(&self) -> f32 {
        unsafe { ZkModelAnimation_getFpsSource(self.ptr.as_ptr()) }
    }
    // pub fn Bbox(&self) -> ZkAxisAlignedBoundingBox { unsafe {ZkModelAnimation_getBbox(self.ptr.as_ptr()).to_string()}}
    pub fn checksum(&self) -> u32 {
        unsafe { ZkModelAnimation_getChecksum(self.ptr.as_ptr()) }
    }
    pub fn source_path(&self) -> String {
        unsafe { ZkModelAnimation_getSourcePath(self.ptr.as_ptr()).to_string() }
    }
    // pub fn SourceDate(&self) -> ZkDate { unsafe {ZkModelAnimation_getSourceDate(self.ptr.as_ptr())}}
    pub fn source_script(&self) -> String {
        unsafe { ZkModelAnimation_getSourceScript(self.ptr.as_ptr()).to_string() }
    }

    pub fn sample_count(&self) -> u64 {
        unsafe { ZkModelAnimation_getSampleCount(self.ptr.as_ptr()) }
    }
    pub fn sample(&self, index: u64) -> AnimationSample {
        unsafe { ZkModelAnimation_getSample(self.ptr.as_ptr(), index).to_own() }
    }
    pub fn samples(&self) -> Vec<AnimationSample> {
        let count = self.sample_count();
        (0..count).map(|i| self.sample(i)).collect()
    }

    pub fn node_indices(&self) -> &[u32] {
        unsafe {
            let mut count: ZkSize = 0;
            let ptr = ZkModelAnimation_getNodeIndices(self.ptr.as_ptr(), &mut count);
            std::slice::from_raw_parts(ptr, count as usize)
        }
    }
}

impl Drop for ModelAnimation<'_> {
    fn drop(&mut self) {
        if self.owned {
            unsafe { ZkModelAnimation_del(self.ptr.as_ptr()) };
        }
    }
}
