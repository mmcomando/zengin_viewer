use crate::conversions::*;
use crate::misc::SoftSkinWeightEntry;
use crate::misc::{Mat4, Vec3};
use crate::mrs_mesh::MrsMesh;
use crate::stream::Read;
use crate::vfs::Vfs;
use ZenKitCAPI_sys::*;
use std::marker::PhantomData;
use std::ptr::NonNull;

#[derive(Debug)]
pub struct Model<'a> {
    ptr: NonNull<ZkModel>,
    _marker: PhantomData<&'a ZkModel>,
    owned: bool,
}

impl Model<'_> {
    pub unsafe fn from_raw_ptr(ptr: *mut ZkModel, owned: bool) -> Option<Self> {
        NonNull::new(ptr).map(|ptr| Self {
            ptr,
            owned,
            _marker: PhantomData,
        })
    }

    pub fn load(read: &Read) -> Option<Self> {
        unsafe {
            let ptr = ZkModel_load(read.as_raw());
            return Self::from_raw_ptr(ptr, true);
        };
    }

    pub fn load_path(path: &str) -> Option<Self> {
        let cstr = str_to_zk(path);
        unsafe {
            let ptr = ZkModel_loadPath(cstr.as_ptr());
            return Self::from_raw_ptr(ptr, true);
        };
    }

    pub fn load_vfs(vfs: &Vfs, name: &str) -> Option<Self> {
        let cstr = str_to_zk(name);
        unsafe {
            let ptr = ZkModel_loadVfs(vfs.get_raw(), cstr.as_ptr());
            return Self::from_raw_ptr(ptr, true);
        };
    }

    pub fn hierarchy(&self) -> ModelHierarchy<'_> {
        unsafe {
            let ptr = ZkModel_getHierarchy(self.ptr.as_ptr());
            return ModelHierarchy::from_raw_ptr(ptr.cast_mut(), false).unwrap();
        };
    }

    pub fn mesh(&self) -> ModelMesh<'_> {
        unsafe {
            let raw = ZkModel_getMesh(self.ptr.as_ptr());
            return ModelMesh::from_raw_ptr(raw.cast_mut(), false).unwrap();
        };
    }
}

impl Drop for Model<'_> {
    fn drop(&mut self) {
        if self.owned {
            unsafe { ZkModel_del(self.ptr.as_ptr()) };
        }
    }
}

#[derive(Debug)]
pub struct ModelMesh<'a> {
    ptr: NonNull<ZkModelMesh>,
    _marker: PhantomData<&'a ZkModelMesh>,
    owned: bool,
}

impl ModelMesh<'_> {
    pub unsafe fn from_raw_ptr(ptr: *mut ZkModelMesh, owned: bool) -> Option<Self> {
        NonNull::new(ptr).map(|ptr| Self {
            ptr,
            owned,
            _marker: PhantomData,
        })
    }

    pub fn load(read: &Read) -> Option<Self> {
        unsafe {
            let ptr = ZkModelMesh_load(read.as_raw());
            return Self::from_raw_ptr(ptr, true);
        };
    }

    pub fn load_path(path: &str) -> Option<Self> {
        let cstr = str_to_zk(path);
        unsafe {
            let ptr = ZkModelMesh_loadPath(cstr.as_ptr());
            return Self::from_raw_ptr(ptr, true);
        };
    }

    pub fn load_vfs(vfs: &Vfs, name: &str) -> Option<Self> {
        let cstr = str_to_zk(name);
        unsafe {
            let ptr = ZkModelMesh_loadVfs(vfs.get_raw(), cstr.as_ptr());
            return Self::from_raw_ptr(ptr, true);
        }
    }

    pub fn mesh_count(&self) -> u64 {
        unsafe { ZkModelMesh_getMeshCount(self.ptr.as_ptr()) }
    }

    pub fn mesh(&self, index: u64) -> SoftSkinMesh<'_> {
        unsafe {
            let ptr = ZkModelMesh_getMesh(self.ptr.as_ptr(), index);
            return SoftSkinMesh::from_raw_ptr(ptr.cast_mut(), true).unwrap();
        }
    }

    pub fn meshes(&self) -> Vec<SoftSkinMesh<'_>> {
        let count = self.mesh_count();
        (0..count).map(|i| self.mesh(i)).collect()
    }

    pub fn enumerate_attachments(&self) -> Vec<(String, MrsMesh<'_>)> {
        let mut enumerated: Vec<(String, MrsMesh)> = Vec::new();

        unsafe {
            extern "C" fn trampoline(
                ctx: *mut std::ffi::c_void,
                name: ZkString,
                mesh: *const ZkMultiResolutionMesh,
            ) -> ZkBool {
                unsafe {
                    let vec = &mut *(ctx.cast::<Vec<(String, MrsMesh)>>());
                    vec.push((
                        name.to_string(),
                        MrsMesh::from_raw_ptr(mesh.cast_mut(), false).unwrap(),
                    ));
                };

                0 // Continue iteration
            }
            ZkModelMesh_enumerateAttachments(
                self.ptr.as_ptr(),
                Some(trampoline),
                std::ptr::from_mut(&mut enumerated).cast::<std::ffi::c_void>(),
            );
        }
        enumerated
    }

    // unsafe extern "C" {
    //     pub fn ZkModelMesh_getAttachmentCount(slf: *const ZkModelMesh) -> ZkSize;
    // }
    // unsafe extern "C" {
    //     pub fn ZkModelMesh_getAttachment(
    //         slf: *const ZkModelMesh,
    //         name: ZkString,
    //     ) -> *const ZkMultiResolutionMesh;
    // }
    // unsafe extern "C" {
    //     pub fn ZkModelMesh_enumerateAttachments(
    //         slf: *const ZkModelMesh,
    //         cb: ZkAttachmentEnumerator,
    //         ctx: *mut ::std::os::raw::c_void,
    //     );
    // }
    // pub type ZkAttachmentEnumerator = ::std::option::Option<
    //     unsafe extern "C" fn(
    //         ctx: *mut ::std::os::raw::c_void,
    //         name: ZkString,
    //         mesh: *const ZkMultiResolutionMesh,
    //     ) -> ZkBool,
    // >;

    // ZkModelMesh_getAttachmentCount(slf: *const ZkModelMesh) -> ZkSize;
    // ZkModelMesh_getAttachment(
    // ZkModelMesh_getChecksum(slf: *const ZkModelMesh) -> u32;
}

impl Drop for ModelMesh<'_> {
    fn drop(&mut self) {
        if self.owned {
            unsafe { ZkModelMesh_del(self.ptr.as_ptr()) };
        }
    }
}

#[derive(Debug)]
pub struct SoftSkinMesh<'a> {
    ptr: NonNull<ZkSoftSkinMesh>,
    _marker: PhantomData<&'a ZkSoftSkinMesh>,
    _owned: bool,
}

impl SoftSkinMesh<'_> {
    pub unsafe fn from_raw_ptr(ptr: *mut ZkSoftSkinMesh, owned: bool) -> Option<Self> {
        NonNull::new(ptr).map(|ptr| Self {
            ptr,
            _owned: owned,
            _marker: PhantomData,
        })
    }

    pub fn mesh(&self) -> MrsMesh<'_> {
        unsafe {
            let ptr = ZkSoftSkinMesh_getMesh(self.ptr.as_ptr());
            return MrsMesh::from_raw_ptr(ptr.cast_mut(), false).unwrap();
        };
    }

    pub fn weight_total(&self) -> u64 {
        unsafe { ZkSoftSkinMesh_getWeightTotal(self.ptr.as_ptr()) }
    }
    pub fn weight_count(&self, node_index: u64) -> u64 {
        unsafe { ZkSoftSkinMesh_getWeightCount(self.ptr.as_ptr(), node_index) }
    }

    pub fn weight(&self, vertex_index: u64, weight_index: u64) -> SoftSkinWeightEntry {
        unsafe { ZkSoftSkinMesh_getWeight(self.ptr.as_ptr(), vertex_index, weight_index).to_own() }
    }

    pub fn node_count(&self) -> u64 {
        unsafe { ZkSoftSkinMesh_getNodeCount(self.ptr.as_ptr()) }
    }

    pub fn nodes(&self) -> &[i32] {
        unsafe {
            let mut count: ZkSize = 0;
            let ptr = ZkSoftSkinMesh_getNodes(self.ptr.as_ptr(), &mut count);
            std::slice::from_raw_parts(ptr, count as usize)
        }
    }
}

// ZkSoftSkinMesh_getBbox(
// ZkSoftSkinMesh_getWedgeNormalCount(slf: *const ZkSoftSkinMesh) -> ZkSize;
// ZkSoftSkinMesh_getWedgeNormal(

pub struct ModelHierarchyNode {
    pub parent_index: i16,
    pub name: String,
    pub transform: Mat4,
}

#[derive(Debug)]
pub struct ModelHierarchy<'a> {
    ptr: NonNull<ZkModelHierarchy>,
    _marker: PhantomData<&'a ZkModelHierarchy>,
    owned: bool,
}

impl ModelHierarchy<'_> {
    pub unsafe fn from_raw_ptr(ptr: *mut ZkModelHierarchy, owned: bool) -> Option<Self> {
        NonNull::new(ptr).map(|ptr| Self {
            ptr,
            owned,
            _marker: PhantomData,
        })
    }

    pub fn load(read: &Read) -> Option<Self> {
        unsafe {
            let ptr = ZkModelHierarchy_load(read.as_raw());
            return Self::from_raw_ptr(ptr, true);
        };
    }

    pub fn load_path(path: &str) -> Option<Self> {
        let cstr = str_to_zk(path);
        unsafe {
            let ptr = ZkModelHierarchy_loadPath(cstr.as_ptr());
            return Self::from_raw_ptr(ptr, true);
        };
    }

    pub fn load_vfs(vfs: &Vfs, name: &str) -> Option<Self> {
        let cstr = str_to_zk(name);
        unsafe {
            let ptr = ZkModelHierarchy_loadVfs(vfs.get_raw(), cstr.as_ptr());
            return Self::from_raw_ptr(ptr, true);
        }
    }

    pub fn node_count(&self) -> u64 {
        unsafe { ZkModelHierarchy_getNodeCount(self.ptr.as_ptr()) }
    }

    pub fn node(&self, index: u64) -> ModelHierarchyNode {
        unsafe {
            let ptr = ZkModelHierarchy_getNode(self.ptr.as_ptr(), index);
            return ModelHierarchyNode {
                parent_index: ptr.parentIndex,
                name: ptr.name.to_string(),
                transform: ptr.transform.to_own(),
            };
        }
    }

    pub fn nodes(&self) -> Vec<ModelHierarchyNode> {
        let count = self.node_count();
        (0..count).map(|i| self.node(i)).collect()
    }

    pub fn root_translation(&self) -> Vec3 {
        unsafe { ZkModelHierarchy_getRootTranslation(self.ptr.as_ptr()).to_own() }
    }
    pub fn source_path(&self) -> String {
        unsafe { ZkModelHierarchy_getSourcePath(self.ptr.as_ptr()).to_string() }
    }
}
impl Drop for ModelHierarchy<'_> {
    fn drop(&mut self) {
        if self.owned {
            unsafe { ZkModelHierarchy_del(self.ptr.as_ptr()) };
        }
    }
}

// ZkModelHierarchy_getBbox(slf: *const ZkModelHierarchy) -> ZkAxisAlignedBoundingBox;
// ZkModelHierarchy_getCollisionBbox(
// ZkModelHierarchy_getChecksum(slf: *const ZkModelHierarchy) -> u32;
// ZkModelHierarchy_getSourceDate(slf: *const ZkModelHierarchy) -> ZkDate;
