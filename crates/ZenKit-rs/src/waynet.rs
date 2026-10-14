use crate::conversions::*;

use crate::misc::Vec3;
use ZenKitCAPI_sys::*;
use std::marker::PhantomData;
use std::ptr::NonNull;

#[derive(Debug)]
pub struct WayNet<'a> {
    ptr: NonNull<ZkWayNet>,
    _marker: PhantomData<&'a ZkWayNet>,
    _owned: bool,
}

impl WayNet<'_> {
    pub unsafe fn from_raw_ptr(ptr: *mut ZkWayNet, owned: bool) -> Option<Self> {
        NonNull::new(ptr).map(|ptr| Self {
            ptr,
            _owned: owned,
            _marker: PhantomData,
        })
    }

    pub fn point_count(&self) -> u64 {
        unsafe { ZkWayNet_getPointCount(self.ptr.as_ptr()) }
    }

    pub fn point(&self, index: u64) -> WayNetPoint<'_> {
        unsafe {
            let ptr = ZkWayNet_getPoint(self.ptr.as_ptr(), index);
            return WayNetPoint::from_raw_ptr(ptr.cast_mut(), false).unwrap();
        }
    }

    pub fn points(&self) -> Vec<WayNetPoint<'_>> {
        let count = self.point_count();
        (0..count).map(|i| self.point(i)).collect()
    }

    pub fn edge_count(&self) -> u64 {
        unsafe { ZkWayNet_getEdgeCount(self.ptr.as_ptr()) }
    }

    pub fn edge(&self, index: u64) -> WayEdge<'_> {
        unsafe {
            let ptr = ZkWayNet_getEdge(self.ptr.as_ptr(), index);
            return WayEdge::from_raw_ptr(ptr.cast_mut(), false).unwrap();
        }
    }

    pub fn edges(&self) -> Vec<WayEdge<'_>> {
        let count = self.edge_count();
        (0..count).map(|i| self.edge(i)).collect()
    }
}

#[derive(Debug)]
pub struct WayEdge<'a> {
    ptr: NonNull<ZkWayEdge>,
    _marker: PhantomData<&'a ZkWayEdge>,
    _owned: bool,
}

impl WayEdge<'_> {
    pub unsafe fn from_raw_ptr(ptr: *mut ZkWayEdge, owned: bool) -> Option<Self> {
        NonNull::new(ptr).map(|ptr| Self {
            ptr,
            _marker: PhantomData,
            _owned: owned,
        })
    }

    pub fn start_point(&self) -> WayNetPoint<'_> {
        unsafe {
            let ptr = ZkWayEdge_getStartPoint(self.ptr.as_ptr().cast_const());
            return WayNetPoint::from_raw_ptr(ptr.cast_mut(), false).unwrap();
        }
    }

    pub fn end_point(&self) -> WayNetPoint<'_> {
        unsafe {
            let ptr = ZkWayEdge_getEndPoint(self.ptr.as_ptr().cast_const());
            return WayNetPoint::from_raw_ptr(ptr.cast_mut(), false).unwrap();
        }
    }
}

#[derive(Debug)]
pub struct WayNetPoint<'a> {
    ptr: NonNull<ZkWayPoint>,
    _marker: PhantomData<&'a ZkWayPoint>,
    _owned: bool,
}

impl WayNetPoint<'_> {
    pub unsafe fn from_raw_ptr(ptr: *mut ZkWayPoint, owned: bool) -> Option<Self> {
        NonNull::new(ptr).map(|ptr| Self {
            ptr,
            _owned: owned,
            _marker: PhantomData,
        })
    }

    pub fn name(&self) -> String {
        unsafe { ZkWayPoint_getName(self.ptr.as_ptr().cast_const()).to_string() }
    }
    pub fn water_depth(&self) -> i32 {
        unsafe { ZkWayPoint_getWaterDepth(self.ptr.as_ptr().cast_const()) }
    }
    pub fn under_water(&self) -> bool {
        unsafe { ZkWayPoint_getUnderWater(self.ptr.as_ptr().cast_const()).to_bool() }
    }
    pub fn position(&self) -> Vec3 {
        unsafe { ZkWayPoint_getPosition(self.ptr.as_ptr().cast_const()).to_own() }
    }
    pub fn direction(&self) -> Vec3 {
        unsafe { ZkWayPoint_getDirection(self.ptr.as_ptr().cast_const()).to_own() }
    }
    pub fn free_point(&self) -> bool {
        unsafe { ZkWayPoint_getFreePoint(self.ptr.as_ptr().cast_const()).to_bool() }
    }
}
