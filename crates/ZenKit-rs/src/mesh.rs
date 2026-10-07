use crate::conversions::*;
use crate::material::Material;
use crate::misc::{Vec2, Vec3};
use crate::obb::OrientedBoundingBox;
use crate::stream::Read;
use crate::texture::Texture;
use crate::vfs::Vfs;
use ZenKitCAPI_sys::*;
use std::marker::PhantomData;
use std::ptr::NonNull;

/// Represents a vertex feature.
pub struct VertexFeature {
    /// The texture coordinates of the polygon.
    pub texture: Vec2,
    /// A value indicating the light level of the polygon.
    pub light: u32,
    /// The normal vector of the polygon.
    pub normal: Vec3,
}

#[derive(Debug)]
pub struct Mesh<'a> {
    ptr: NonNull<ZkMesh>,
    _marker: PhantomData<&'a ZkMesh>,
    owned: bool,
}

impl Mesh<'_> {
    pub unsafe fn from_raw_ptr(ptr: *mut ZkMesh, owned: bool) -> Option<Self> {
        NonNull::new(ptr).map(|ptr| Self {
            ptr,
            owned,
            _marker: PhantomData,
        })
    }

    pub fn load(read: &Read) -> Option<Self> {
        unsafe {
            let raw = ZkMesh_load(read.as_raw());
            return Self::from_raw_ptr(raw, true);
        };
    }

    pub fn load_path(path: &str) -> Option<Self> {
        let cstr = str_to_zk(path);
        unsafe {
            let raw = ZkMesh_loadPath(cstr.as_ptr());
            return Self::from_raw_ptr(raw, true);
        };
    }

    pub fn load_vfs(vfs: &Vfs, name: &str) -> Option<Self> {
        let cstr = str_to_zk(name);
        unsafe {
            let raw = ZkMesh_loadVfs(vfs.get_raw(), cstr.as_ptr());
            return Self::from_raw_ptr(raw, true);
        };
    }

    pub fn name(&self) -> String {
        unsafe {
            let ptr = ZkMesh_getName(self.ptr.as_ptr());
            ptr.to_string()
        }
    }

    pub fn source_date(&self) -> ZkDate {
        unimplemented!()
        // unsafe { ZkMesh_getSourceDate(self.ptr.as_ptr()) }
    }

    pub fn bounding_box(&self) -> ZkAxisAlignedBoundingBox {
        unimplemented!()
        // unsafe { ZkMesh_getBoundingBox(self.ptr.as_ptr()) }
    }

    pub fn oriented_bbox(&self) -> OrientedBoundingBox<'_> {
        unsafe {
            let ptr = ZkMesh_getOrientedBoundingBox(self.ptr.as_ptr());
            return OrientedBoundingBox::from_raw_ptr(ptr.cast_mut()).unwrap();
        }
    }

    pub fn material_count(&self) -> u64 {
        unsafe { ZkMesh_getMaterialCount(self.ptr.as_ptr()) }
    }

    pub fn material(&self, index: u64) -> Material<'_> {
        let ptr = unsafe { ZkMesh_getMaterial(self.ptr.as_ptr(), index) };
        Material::from_raw(ptr.cast_mut()).unwrap()
    }

    pub fn materials(&self) -> Vec<Material<'_>> {
        let count = self.material_count();
        (0..count).map(|i| self.material(i)).collect()
    }

    pub fn position_count(&self) -> u64 {
        unsafe { ZkMesh_getPositionCount(self.ptr.as_ptr()) }
    }

    pub fn position(&self, index: u64) -> Vec3 {
        unsafe { ZkMesh_getPosition(self.ptr.as_ptr(), index).to_own() }
    }

    pub fn positions(&self) -> Vec<Vec3> {
        let count = self.position_count();
        (0..count).map(|i| self.position(i)).collect()
    }

    pub fn vertex_count(&self) -> u64 {
        unsafe { ZkMesh_getVertexCount(self.ptr.as_ptr()) }
    }

    pub fn vertex(&self, index: u64) -> VertexFeature {
        let zk_vertex = unsafe { ZkMesh_getVertex(self.ptr.as_ptr(), index) };
        VertexFeature {
            texture: zk_vertex.texture.to_own(),
            light: zk_vertex.light,
            normal: zk_vertex.normal.to_own(),
        }
    }

    pub fn vertices(&self) -> Vec<VertexFeature> {
        let count = self.vertex_count();
        (0..count).map(|i| self.vertex(i)).collect()
    }

    pub fn polygon_count(&self) -> u64 {
        unsafe { ZkMesh_getPolygonCount(self.ptr.as_ptr()) }
    }

    pub fn polygon(&self, index: u64) -> Polygon<'_> {
        let ptr = unsafe { ZkMesh_getPolygon(self.ptr.as_ptr(), index) };
        NonNull::new(ptr.cast_mut())
            .map(|p| Polygon { ptr: p, mesh: self })
            .unwrap()
    }

    pub fn polygons(&self) -> Vec<Polygon<'_>> {
        let count = self.polygon_count();
        (0..count).map(|i| self.polygon(i)).collect()
    }

    pub fn light_map_count(&self) -> u64 {
        unsafe { ZkMesh_getLightMapCount(self.ptr.as_ptr()) }
    }

    pub fn light_map(&self, index: u64) -> LightMap<'_> {
        let ptr = unsafe { ZkMesh_getLightMap(self.ptr.as_ptr(), index) };
        NonNull::new(ptr.cast_mut())
            .map(|p| LightMap {
                ptr: p,
                _marker: PhantomData,
            })
            .unwrap()
    }

    pub fn light_maps(&self) -> Vec<LightMap<'_>> {
        let count = self.light_map_count();
        (0..count).map(|i| self.light_map(i)).collect()
    }
}

impl Drop for Mesh<'_> {
    fn drop(&mut self) {
        if self.owned {
            unsafe { ZkMesh_del(self.ptr.as_ptr()) };
        }
    }
}

pub struct LightMap<'a> {
    ptr: NonNull<ZkLightMap>,
    _marker: PhantomData<&'a ZkLightMap>,
}

impl<'a> LightMap<'a> {
    pub fn image(&self) -> Texture<'a> {
        unsafe {
            let ptr = ZkLightMap_getImage(self.ptr.as_ptr());
            return Texture::from_raw_ptr(ptr.cast_mut(), false).unwrap();
        }
    }
    pub fn origin(&self) -> Vec3 {
        unsafe { ZkLightMap_getOrigin(self.ptr.as_ptr()).to_own() }
    }
    pub fn normal(&self, index: usize) -> Vec3 {
        unsafe { ZkLightMap_getNormal(self.ptr.as_ptr(), index as u64).to_own() }
    }
}

pub struct Polygon<'a> {
    ptr: NonNull<ZkPolygon>,
    mesh: &'a Mesh<'a>,
}

impl Polygon<'_> {
    pub fn is_portal(&self) -> bool {
        unsafe { ZkPolygon_getIsPortal(self.ptr.as_ptr()).to_bool() }
    }
    pub fn is_occluder(&self) -> bool {
        unsafe { ZkPolygon_getIsOccluder(self.ptr.as_ptr()).to_bool() }
    }
    pub fn is_sector(&self) -> bool {
        unsafe { ZkPolygon_getIsSector(self.ptr.as_ptr()).to_bool() }
    }
    pub fn is_should_relight(&self) -> bool {
        unsafe { ZkPolygon_getShouldRelight(self.ptr.as_ptr()).to_bool() }
    }
    pub fn is_outdoor(&self) -> bool {
        unsafe { ZkPolygon_getIsOutdoor(self.ptr.as_ptr()).to_bool() }
    }
    pub fn is_ghost_occluder(&self) -> bool {
        unsafe { ZkPolygon_getIsGhostOccluder(self.ptr.as_ptr()).to_bool() }
    }
    pub fn is_dynamically_lit(&self) -> bool {
        unsafe { ZkPolygon_getIsDynamicallyLit(self.ptr.as_ptr()).to_bool() }
    }
    pub fn is_lod(&self) -> bool {
        unsafe { ZkPolygon_getIsLod(self.ptr.as_ptr()).to_bool() }
    }

    pub fn material_index(&self) -> u32 {
        unsafe { ZkPolygon_getMaterialIndex(self.ptr.as_ptr()) }
    }

    pub fn light_map_index(&self) -> i32 {
        unsafe { ZkPolygon_getLightMapIndex(self.ptr.as_ptr()) }
    }

    pub fn normal_axis(&self) -> u8 {
        unsafe { ZkPolygon_getNormalAxis(self.ptr.as_ptr()) }
    }

    pub fn sector_ndex(&self) -> i16 {
        unsafe { ZkPolygon_getSectorIndex(self.ptr.as_ptr()) }
    }

    pub fn position_indices(&self) -> &[u32] {
        unsafe {
            let mut count: ZkSize = 0;
            let ptr =
                ZkPolygon_getPositionIndices(self.ptr.as_ptr(), self.mesh.ptr.as_ptr(), &mut count);
            std::slice::from_raw_parts(ptr, count as usize)
        }
    }
    pub fn feature_indices(&self) -> &[u32] {
        unsafe {
            let mut count: ZkSize = 0;
            let ptr =
                ZkPolygon_getFeatureIndices(self.ptr.as_ptr(), self.mesh.ptr.as_ptr(), &mut count);
            std::slice::from_raw_parts(ptr, count as usize)
        }
    }
}
