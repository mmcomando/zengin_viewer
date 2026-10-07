use crate::conversions::*;

use crate::misc::Vec3;
use ZenKitCAPI_sys::*;
use std::marker::PhantomData;
use std::ptr::NonNull;

#[derive(Debug)]
pub struct OrientedBoundingBox<'a> {
    ptr: NonNull<ZkOrientedBoundingBox>,
    _marker: PhantomData<&'a ZkModelScript>,
}

impl OrientedBoundingBox<'_> {
    pub unsafe fn from_raw_ptr(ptr: *mut ZkOrientedBoundingBox) -> Option<Self> {
        NonNull::new(ptr).map(|ptr| Self {
            ptr,
            _marker: PhantomData,
        })
    }

    pub fn center(&self) -> Vec3 {
        unsafe { ZkOrientedBoundingBox_getCenter(self.ptr.as_ptr()).to_own() }
    }
    pub fn axis(&self, index: u64) -> Vec3 {
        unsafe { ZkOrientedBoundingBox_getAxis(self.ptr.as_ptr(), index).to_own() }
    }
    pub fn half_width(&self) -> Vec3 {
        unsafe { ZkOrientedBoundingBox_getHalfWidth(self.ptr.as_ptr()).to_own() }
    }
    pub fn child_count(&self) -> u64 {
        unsafe { ZkOrientedBoundingBox_getChildCount(self.ptr.as_ptr()) }
    }
}
