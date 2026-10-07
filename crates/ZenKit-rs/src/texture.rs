use ZenKitCAPI_sys::*;
use std::{marker::PhantomData, ptr::NonNull};

use crate::conversions::*;
use crate::misc::*;
use crate::stream::Read;
use crate::vfs::Vfs;

pub struct Texture<'a> {
    ptr: NonNull<ZkTexture>,
    _marker: PhantomData<&'a ZkTexture>,
    owned: bool,
}

impl Texture<'_> {
    /// Safety: owned value should have owned true, view to data should have owned set to false
    pub unsafe fn from_raw_ptr(ptr: *mut ZkTexture, owned: bool) -> Option<Self> {
        NonNull::new(ptr).map(|ptr| Self {
            ptr,
            owned,
            _marker: PhantomData,
        })
    }

    pub fn load(reader: &Read) -> Option<Self> {
        unsafe {
            let ptr = ZkTexture_load(reader.as_raw());
            return Self::from_raw_ptr(ptr, true);
        }
    }

    pub fn load_path(path: &str) -> Option<Self> {
        let cstr = str_to_zk(path);
        unsafe {
            let raw = ZkTexture_loadPath(cstr.as_ptr());
            return Self::from_raw_ptr(raw, true);
        };
    }

    pub fn load_vfs(vfs: &Vfs, name: &str) -> Option<Self> {
        let cstr = str_to_zk(name);
        unsafe {
            let raw = ZkTexture_loadVfs(vfs.get_raw(), cstr.as_ptr());
            return Self::from_raw_ptr(raw, true);
        };
    }

    pub fn width(&self) -> u32 {
        unsafe { ZkTexture_getWidth(self.ptr.as_ptr()) }
    }
    pub fn height(&self) -> u32 {
        unsafe { ZkTexture_getHeight(self.ptr.as_ptr()) }
    }
    pub fn width_ref(&self) -> u32 {
        unsafe { ZkTexture_getWidthRef(self.ptr.as_ptr()) }
    }
    pub fn height_ref(&self) -> u32 {
        unsafe { ZkTexture_getHeightRef(self.ptr.as_ptr()) }
    }
    pub fn mipmap_count(&self) -> u32 {
        unsafe { ZkTexture_getMipmapCount(self.ptr.as_ptr()) }
    }
    pub fn average_color(&self) -> u32 {
        unsafe { ZkTexture_getAverageColor(self.ptr.as_ptr()) }
    }
    pub fn palette_size(&self) -> u64 {
        unsafe { ZkTexture_getPaletteSize(self.ptr.as_ptr()) }
    }

    pub fn width_mipmap(&self, level: u64) -> u32 {
        unsafe { ZkTexture_getWidthMipmap(self.ptr.as_ptr(), level) }
    }
    pub fn height_mipmap(&self, level: u64) -> u32 {
        unsafe { ZkTexture_getHeightMipmap(self.ptr.as_ptr(), level) }
    }

    pub fn get_format(&self) -> TextureFormat {
        unsafe { ZkTexture_getFormat(self.ptr.as_ptr()) }
    }

    pub fn mipmap_rgba(&self, level: u64) -> Vec<u8> {
        let one_value_size = 4;
        let size = one_value_size * (self.width_mipmap(level) * self.height_mipmap(level)) as usize;
        let mut data = Vec::with_capacity(size);
        unsafe {
            data.set_len(size);
            ZkTexture_getMipmapRgba(
                self.ptr.as_ptr(),
                level,
                data.as_mut_ptr(),
                data.len() as u64,
            );
        }
        return data;
    }
}

impl Drop for Texture<'_> {
    fn drop(&mut self) {
        if self.owned {
            unsafe { ZkTexture_del(self.ptr.as_ptr()) }
        }
    }
}

// TODO
// ZkTexture_getPaletteItem
// ZkTexture_enumeratePaletteItems
// ZkTexture_getMipmapRaw(
// ZkTexture_enumerateRawMipmaps(
