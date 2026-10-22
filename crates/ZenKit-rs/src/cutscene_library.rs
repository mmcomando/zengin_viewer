use ZenKitCAPI_sys::*;
use std::{marker::PhantomData, ptr::NonNull};

use crate::conversions::*;
use crate::misc::TextEncoding;
use crate::stream::Read;
use crate::vfs::Vfs;

pub struct CutsceneLibrary<'a> {
    ptr: NonNull<ZkCutsceneLibrary>,
    _marker: PhantomData<&'a ZkCutsceneLibrary>,
    owned: bool,
    encoding: TextEncoding,
}

impl CutsceneLibrary<'_> {
    /// Safety: owned value should have owned true, view to data should have owned set to false
    pub unsafe fn from_raw_ptr(ptr: *mut ZkCutsceneLibrary, owned: bool) -> Option<Self> {
        unsafe { Self::from_raw_ptr_with_encoding(ptr, owned, TextEncoding::Utf8) }
    }

    unsafe fn from_raw_ptr_with_encoding(
        ptr: *mut ZkCutsceneLibrary,
        owned: bool,
        encoding: TextEncoding,
    ) -> Option<Self> {
        NonNull::new(ptr).map(|ptr| Self {
            ptr,
            owned,
            _marker: PhantomData,
            encoding,
        })
    }

    pub fn load(reader: &Read) -> Option<Self> {
        Self::load_with_encoding(reader, TextEncoding::Utf8)
    }

    /// Loads the library, decoding message strings from `encoding` into UTF-8.
    ///
    /// Ou.bin stores message strings in the code page of the game language
    /// edition, e.g. windows-1252 for western or windows-1250 for central
    /// European releases.
    pub fn load_with_encoding(reader: &Read, encoding: TextEncoding) -> Option<Self> {
        unsafe {
            let ptr = ZkCutsceneLibrary_load(reader.as_raw());
            return Self::from_raw_ptr_with_encoding(ptr, true, encoding);
        }
    }

    pub fn load_path(path: &str) -> Option<Self> {
        let cstr = str_to_zk(path);
        unsafe {
            let raw = ZkCutsceneLibrary_loadPath(cstr.as_ptr());
            return Self::from_raw_ptr(raw, true);
        };
    }

    pub fn load_vfs(vfs: &Vfs, name: &str) -> Option<Self> {
        let cstr = str_to_zk(name);
        unsafe {
            let raw = ZkCutsceneLibrary_loadVfs(vfs.get_raw(), cstr.as_ptr());
            return Self::from_raw_ptr(raw, true);
        };
    }

    pub fn block_count(&self) -> u64 {
        unsafe { ZkCutsceneLibrary_getBlockCount(self.ptr.as_ptr()) }
    }

    pub fn block(&self, name: &str) -> Option<CutsceneBlock<'_>> {
        let cstr = str_to_zk(name);
        unsafe {
            let raw = ZkCutsceneLibrary_getBlock(self.ptr.as_ptr(), cstr.as_ptr());
            return CutsceneBlock::from_raw_ptr_with_encoding(raw.cast_mut(), false, self.encoding);
        }
    }

    pub fn block_by_index(&self, index: u64) -> Option<CutsceneBlock<'_>> {
        unsafe {
            let raw = ZkCutsceneLibrary_getBlockByIndex(self.ptr.as_ptr(), index);
            return CutsceneBlock::from_raw_ptr_with_encoding(raw.cast_mut(), false, self.encoding);
        }
    }

    pub fn blocks(&self) -> Vec<CutsceneBlock<'_>> {
        let mut raw_blocks: Vec<NonNull<ZkCutsceneBlock>> = Vec::new();

        unsafe {
            extern "C" fn trampoline(
                ctx: *mut std::ffi::c_void,
                block: *const ZkCutsceneBlock,
            ) -> ZkBool {
                let vec = unsafe { &mut *ctx.cast::<Vec<NonNull<ZkCutsceneBlock>>>() };

                if let Some(nn) = NonNull::new(block.cast_mut()) {
                    vec.push(nn);
                }
                0 // Continue iteration
            }
            ZkCutsceneLibrary_enumerateBlocks(
                self.ptr.as_ptr(),
                Some(trampoline),
                std::ptr::from_mut(&mut raw_blocks).cast::<std::ffi::c_void>(),
            );
        }

        raw_blocks
            .into_iter()
            .map(|ptr| CutsceneBlock {
                ptr,
                _marker: PhantomData,
                _owned: false,
                encoding: self.encoding,
            })
            .collect()
    }
}

impl Drop for CutsceneLibrary<'_> {
    fn drop(&mut self) {
        if self.owned {
            unsafe { ZkCutsceneLibrary_del(self.ptr.as_ptr()) }
        }
    }
}

pub struct CutsceneBlock<'a> {
    ptr: NonNull<ZkCutsceneBlock>,
    _marker: PhantomData<&'a ZkCutsceneBlock>,
    _owned: bool,
    encoding: TextEncoding,
}

impl CutsceneBlock<'_> {
    /// Safety: owned value should have owned true, view to data should have owned set to false
    pub unsafe fn from_raw_ptr(ptr: *mut ZkCutsceneBlock, owned: bool) -> Option<Self> {
        unsafe { Self::from_raw_ptr_with_encoding(ptr, owned, TextEncoding::Utf8) }
    }

    unsafe fn from_raw_ptr_with_encoding(
        ptr: *mut ZkCutsceneBlock,
        owned: bool,
        encoding: TextEncoding,
    ) -> Option<Self> {
        NonNull::new(ptr).map(|ptr| Self {
            ptr,
            _owned: owned,
            _marker: PhantomData,
            encoding,
        })
    }

    pub fn name(&self) -> String {
        unsafe { ZkCutsceneBlock_getName(self.ptr.as_ptr()).to_string() }
    }

    pub fn message(&self) -> Option<CutsceneMessage<'_>> {
        unsafe {
            let raw = ZkCutsceneBlock_getMessage(self.ptr.as_ptr());
            return CutsceneMessage::from_raw_ptr_with_encoding(
                raw.cast_mut(),
                false,
                self.encoding,
            );
        }
    }
}

pub struct CutsceneMessage<'a> {
    ptr: NonNull<ZkCutsceneMessage>,
    _marker: PhantomData<&'a ZkCutsceneMessage>,
    _owned: bool,
    encoding: TextEncoding,
}

impl CutsceneMessage<'_> {
    /// Safety: owned value should have owned true, view to data should have owned set to false
    pub unsafe fn from_raw_ptr(ptr: *mut ZkCutsceneMessage, owned: bool) -> Option<Self> {
        unsafe { Self::from_raw_ptr_with_encoding(ptr, owned, TextEncoding::Utf8) }
    }

    unsafe fn from_raw_ptr_with_encoding(
        ptr: *mut ZkCutsceneMessage,
        owned: bool,
        encoding: TextEncoding,
    ) -> Option<Self> {
        NonNull::new(ptr).map(|ptr| Self {
            ptr,
            _owned: owned,
            _marker: PhantomData,
            encoding,
        })
    }

    pub fn get_type(&self) -> u32 {
        unsafe { ZkCutsceneMessage_getType(self.ptr.as_ptr()) }
    }
    pub fn text(&self) -> String {
        unsafe {
            ZkCutsceneMessage_getText(self.ptr.as_ptr()).to_string_with_encoding(self.encoding)
        }
    }
    pub fn name(&self) -> String {
        unsafe {
            ZkCutsceneMessage_getName(self.ptr.as_ptr()).to_string_with_encoding(self.encoding)
        }
    }
    pub fn is_high_priority(&self) -> bool {
        unsafe { ZkCutsceneMessage_getIsHighPriority(self.ptr.as_ptr()).to_bool() }
    }
    pub fn is_used(&self) -> bool {
        unsafe { ZkCutsceneMessage_getIsUsed(self.ptr.as_ptr()).to_bool() }
    }
    pub fn is_deleted(&self) -> bool {
        unsafe { ZkCutsceneMessage_getIsDeleted(self.ptr.as_ptr()).to_bool() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing_common::*;

    fn cutscene_file() -> String {
        gothic2_dir() + "/_work/Data/Scripts/CONTENT/CUTSCENE/Ou.bin"
    }

    #[test]
    fn read_cutscene_library() {
        let _zenkit = zenkit_ffi();
        let library = CutsceneLibrary::load_path(&cutscene_file()).unwrap();
        assert!(library.block_count() > 0);

        let first = library.block_by_index(0).unwrap();
        assert!(!first.name().is_empty());

        let message = first.message().unwrap();
        assert!(!message.name().is_empty());
    }

    #[test]
    fn read_cutscene_library_from_memory() {
        let _zenkit = zenkit_ffi();
        let data = std::fs::read(cutscene_file()).expect("should read cutscene library file");
        let reader = Read::from_slice(&data).unwrap();
        let library = CutsceneLibrary::load(&reader).unwrap();
        assert!(library.block_count() > 0);
    }

    #[test]
    fn read_cutscene_library_with_encoding() {
        let _zenkit = zenkit_ffi();
        let data = std::fs::read(cutscene_file()).expect("should read cutscene library file");
        let reader = Read::from_slice(&data).unwrap();
        let library =
            CutsceneLibrary::load_with_encoding(&reader, TextEncoding::Windows1252).unwrap();
        assert!(library.block_count() > 0);

        let first = library.block_by_index(0).unwrap();
        let message = first.message().unwrap();
        assert!(!message.name().is_empty());
        assert!(!message.text().is_empty());
    }

    #[test]
    fn enumerate_blocks() {
        let _zenkit = zenkit_ffi();
        let library = CutsceneLibrary::load_path(&cutscene_file()).unwrap();

        let blocks = library.blocks();
        assert_eq!(blocks.len(), library.block_count() as usize);
        assert_eq!(blocks[0].name(), library.block_by_index(0).unwrap().name());
    }
}

// ZkCutsceneLibrary_save
// ZkCutsceneLibrary_savePath
// ZkCutsceneBlock_setMessage
// ZkCutsceneBlock_release
// ZkCutsceneMessage_setType
// ZkCutsceneMessage_setText
// ZkCutsceneMessage_setName
// ZkCutsceneMessage_setIsHighPriority
// ZkCutsceneMessage_setIsUsed
// ZkCutsceneMessage_setIsDeleted
// ZkCutsceneMessage_release
// ZkCutsceneProps_*
// ZkCutsceneContext_*
