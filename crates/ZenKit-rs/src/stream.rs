use ZenKitCAPI_sys::*;
use std::{os::raw::c_void, ptr::NonNull};

#[derive(Debug)]
pub struct Read {
    ptr: NonNull<ZkRead>,
}

impl Read {
    pub fn from_raw(raw: *mut ZkRead) -> Option<Self> {
        let ptr = NonNull::new(raw)?;
        Some(Self { ptr })
    }

    pub fn from_slice(bytes: &[u8]) -> Option<Self> {
        let raw = unsafe { ZkRead_newMem(bytes.as_ptr(), bytes.len() as ZkSize) };
        let ptr = NonNull::new(raw)?;
        Some(Self { ptr })
    }

    pub fn as_raw(&self) -> *mut ZkRead {
        self.ptr.as_ptr()
    }

    pub fn size(&self) -> u64 {
        unsafe { ZkRead_getSize(self.ptr.as_ptr()) }
    }

    pub fn bytes(&self) -> Vec<u8> {
        let size = self.size();
        let mut data: Vec<u8> = Vec::with_capacity(size as usize);
        unsafe {
            data.set_len(size as usize);
            let read_bytes =
                ZkRead_getBytes(self.ptr.as_ptr(), data.as_mut_ptr().cast::<c_void>(), size);
            assert_eq!(size, read_bytes);
        }
        data
    }
}

impl Drop for Read {
    fn drop(&mut self) {
        unsafe {
            ZkRead_del(self.ptr.as_ptr());
        }
    }
}
