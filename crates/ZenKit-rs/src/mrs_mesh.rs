use crate::conversions::*;
use crate::material::Material;
use crate::misc::{Vec2, Vec3};
use crate::obb::OrientedBoundingBox;
use crate::stream::Read;
use crate::vfs::Vfs;
use ZenKitCAPI_sys::*;
use std::marker::PhantomData;
use std::ptr::NonNull;

pub type MeshTriangle = ZkMeshTriangle;
pub struct MeshWedge {
    pub normal: Vec3,
    pub texture: Vec2,
    pub index: u16,
}
#[derive(Debug)]
pub struct MrsMesh<'a> {
    ptr: NonNull<ZkMultiResolutionMesh>,
    _marker: PhantomData<&'a ZkMultiResolutionMesh>,
    owned: bool,
}

impl MrsMesh<'_> {
    pub unsafe fn from_raw_ptr(ptr: *mut ZkMultiResolutionMesh, owned: bool) -> Option<Self> {
        NonNull::new(ptr).map(|ptr| Self {
            ptr,
            owned,
            _marker: PhantomData,
        })
    }

    pub fn load(read: &Read) -> Option<Self> {
        unsafe {
            let raw = ZkMultiResolutionMesh_load(read.as_raw());
            return Self::from_raw_ptr(raw, true);
        };
    }

    pub fn load_path(path: &str) -> Option<Self> {
        let cstr = str_to_zk(path);
        unsafe {
            let raw = ZkMultiResolutionMesh_loadPath(cstr.as_ptr());
            return Self::from_raw_ptr(raw, true);
        };
    }

    pub fn load_vfs(vfs: &Vfs, name: &str) -> Option<Self> {
        let cstr = str_to_zk(name);
        unsafe {
            let raw = ZkMultiResolutionMesh_loadVfs(vfs.get_raw(), cstr.as_ptr());
            return Self::from_raw_ptr(raw, true);
        };
    }

    pub fn material_count(&self) -> u64 {
        unsafe { ZkMultiResolutionMesh_getMaterialCount(self.ptr.as_ptr()) }
    }

    pub fn material(&self, index: u64) -> Material<'_> {
        let ptr = unsafe { ZkMultiResolutionMesh_getMaterial(self.ptr.as_ptr(), index) };
        Material::from_raw(ptr.cast_mut()).unwrap()
    }

    pub fn materials(&self) -> Vec<Material<'_>> {
        let count = self.material_count();
        (0..count).map(|i| self.material(i)).collect()
    }

    pub fn position_count(&self) -> u64 {
        unsafe { ZkMultiResolutionMesh_getPositionCount(self.ptr.as_ptr()) }
    }

    pub fn position(&self, index: u64) -> Vec3 {
        unsafe { ZkMultiResolutionMesh_getPosition(self.ptr.as_ptr(), index).to_own() }
    }

    pub fn positions(&self) -> Vec<Vec3> {
        let count = self.position_count();
        (0..count).map(|i| self.position(i)).collect()
    }

    pub fn normal_count(&self) -> u64 {
        unsafe { ZkMultiResolutionMesh_getNormalCount(self.ptr.as_ptr()) }
    }

    pub fn normal(&self, index: u64) -> Vec3 {
        unsafe { ZkMultiResolutionMesh_getNormal(self.ptr.as_ptr(), index).to_own() }
    }

    pub fn normals(&self) -> Vec<Vec3> {
        let count = self.normal_count();
        (0..count).map(|i| self.normal(i)).collect()
    }

    pub fn sub_mesh_count(&self) -> u64 {
        unsafe { ZkMultiResolutionMesh_getSubMeshCount(self.ptr.as_ptr()) }
    }

    pub fn sub_mesh(&self, index: u64) -> SubMesh<'_> {
        unsafe {
            let ptr = ZkMultiResolutionMesh_getSubMesh(self.ptr.as_ptr(), index);
            return SubMesh::from_raw_ptr(ptr.cast_mut(), false).unwrap();
        }
    }

    pub fn sub_meshes(&self) -> Vec<SubMesh<'_>> {
        let count = self.sub_mesh_count();
        (0..count).map(|i| self.sub_mesh(i)).collect()
    }

    pub fn oriented_bbox(&self) -> OrientedBoundingBox<'_> {
        unsafe {
            let ptr = ZkMultiResolutionMesh_getOrientedBbox(self.ptr.as_ptr());
            return OrientedBoundingBox::from_raw_ptr(ptr.cast_mut()).unwrap();
        }
    }
}

impl Drop for MrsMesh<'_> {
    fn drop(&mut self) {
        if self.owned {
            unsafe { ZkMultiResolutionMesh_del(self.ptr.as_ptr()) };
        }
    }
}

// pub fn ZkMultiResolutionMesh_getAlphaTest
// pub fn ZkMultiResolutionMesh_getBbox

#[derive(Debug)]
pub struct SubMesh<'a> {
    ptr: NonNull<ZkSubMesh>,
    _marker: PhantomData<&'a ZkSubMesh>,
    _owned: bool,
}

impl SubMesh<'_> {
    pub unsafe fn from_raw_ptr(ptr: *mut ZkSubMesh, owned: bool) -> Option<Self> {
        NonNull::new(ptr).map(|ptr| Self {
            ptr,
            _owned: owned,
            _marker: PhantomData,
        })
    }

    pub fn material(&self) -> Material<'_> {
        let ptr = unsafe { ZkSubMesh_getMaterial(self.ptr.as_ptr()) };
        Material::from_raw(ptr.cast_mut()).unwrap()
    }

    pub fn triangles(&self) -> &[MeshTriangle] {
        unsafe {
            let mut size: u64 = 0;
            let ptr = ZkSubMesh_getTriangles(self.ptr.as_ptr(), std::ptr::from_mut(&mut size));
            std::slice::from_raw_parts(ptr, size as usize)
        }
    }

    pub fn wedge_count(&self) -> u64 {
        unsafe { ZkSubMesh_getWedgeCount(self.ptr.as_ptr()) }
    }

    pub fn wedge(&self, index: u64) -> MeshWedge {
        unsafe {
            let ww = ZkSubMesh_getWedge(self.ptr.as_ptr(), index);
            return MeshWedge {
                normal: ww.normal.to_own(),
                texture: ww.texture.to_own(),
                index: ww.index,
            };
        }
    }

    pub fn wedges(&self) -> Vec<MeshWedge> {
        let count = self.wedge_count();
        (0..count).map(|i| self.wedge(i)).collect()
    }

    pub fn colors(&self) -> &[f32] {
        unsafe {
            let mut size: u64 = 0;
            let ptr = ZkSubMesh_getColors(self.ptr.as_ptr(), std::ptr::from_mut(&mut size));
            std::slice::from_raw_parts(ptr, size as usize)
        }
    }
}

// ZkSubMesh_getTriangleEdges
// ZkSubMesh_getEdges
// ZkSubMesh_getEdgeScores
// ZkSubMesh_getWedgeMap
// ZkSubMesh_getTrianglePlaneIndices
// ZkSubMesh_getTrianglePlaneCount
// ZkSubMesh_getTrianglePlane
