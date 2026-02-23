use ZenKitCAPI_sys::*;
use bevy::{
    asset::io::{AssetReader, AssetReaderError, PathStream, Reader},
    prelude::*,
};
use bevy::{
    asset::io::{
        AssetSourceBuilder, ReaderNotSeekableError, STACK_FUTURE_SIZE, SeekableReader, StackFuture,
    },
    tasks::futures_lite::{self, AsyncRead},
};
use std::ffi::CString;
use std::{
    path::{Path, PathBuf},
    pin::Pin,
    task::{Context, Poll},
};

pub fn get_gothic_assert_bytes(path: &str) -> Vec<u8> {
    unsafe {
        let vfs = ZkVfs_new();

        ZkVfs_mountDiskHost(
            vfs,
            c"/media/MM_HDD_DATA/SteamLibrary/steamapps/common/Gothic II/Data/Textures.vdf"
                .as_ptr(),
            ZkVfsOverwriteBehavior::ZkVfsOverwriteBehavior_ALL,
        );

        // println!("get_gothic_assert_bytes({path})");
        let path = format!("/{}", &path);
        let node = ZkVfs_resolvePath(vfs, CString::new(path.clone()).unwrap().as_ptr());

        let read_obj = ZkVfsNode_open(node);

        let size = ZkRead_getSize(read_obj);
        if size == 0 {
            println!("get_gothic_assert_bytes({}) has 0 size", &path);
        }

        let mut data = Vec::with_capacity(size as usize);

        data.set_len(size as usize);

        let read_bytes = ZkRead_getBytes(
            read_obj,
            data.as_mut_ptr() as *mut ::std::os::raw::c_void,
            size,
        );
        assert!(size == read_bytes);

        ZkRead_del(read_obj);
        ZkVfs_del(vfs);
        return data;
    }
}

pub fn create_gothic_asset_loader() -> AssetSourceBuilder {
    let loader = AssetSourceBuilder::new(move || Box::new(MyAssetReader {}));
    return loader;
}

pub struct MyAsyncReader {
    data: Vec<u8>,
    position: usize,
}

impl MyAsyncReader {
    pub fn new(data: Vec<u8>) -> Self {
        Self { data, position: 0 }
    }
}

impl AsyncRead for MyAsyncReader {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        if self.position >= self.data.len() {
            return Poll::Ready(Ok(0));
        }

        let remaining = &self.data[self.position..];
        let to_copy = std::cmp::min(remaining.len(), buf.len());
        for (dst, src) in buf.iter_mut().zip(&remaining[..to_copy]) {
            *dst = *src
        }
        self.position += to_copy;

        Poll::Ready(Ok(to_copy))
    }
}

// struct MyReader;
impl Reader for MyAsyncReader {
    fn read_to_end<'a>(
        &'a mut self,
        buf: &'a mut Vec<u8>,
    ) -> StackFuture<'a, std::io::Result<usize>, STACK_FUTURE_SIZE> {
        let future = futures_lite::AsyncReadExt::read_to_end(self, buf);
        StackFuture::from(future)
    }
    fn seekable(&mut self) -> Result<&mut dyn SeekableReader, ReaderNotSeekableError> {
        return Err(ReaderNotSeekableError);
    }
}

struct MyAssetReader;
impl AssetReader for MyAssetReader {
    async fn read<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        let path_str = path.as_os_str().to_str().unwrap();
        let bytes = get_gothic_assert_bytes(path_str);
        let val: Box<dyn Reader> = Box::new(MyAsyncReader::new(bytes));
        Ok(val)
    }
    async fn read_meta<'a>(
        &'a self,
        _path: &'a Path,
    ) -> Result<impl Reader + 'a, AssetReaderError> {
        if false {
            // let meta = AssetActionMinimal::Ignore;
            // let meta_str = ron::to_string(&meta).unwrap();
            // let meta_vec = meta_str.into_bytes();
            // let val: Box<dyn Reader> = Box::new(MyAsyncReader::new(meta_vec));
            let val: Box<dyn Reader> = unimplemented!();
            return Ok(val);
        }
        Err(AssetReaderError::NotFound(PathBuf::new()))
        // Err(BevyError())
    }
    async fn read_directory<'a>(
        &'a self,
        _path: &'a Path,
    ) -> Result<Box<PathStream>, AssetReaderError> {
        unimplemented!()
    }
    async fn is_directory<'a>(&'a self, _path: &'a Path) -> Result<bool, AssetReaderError> {
        Ok(false)
    }
    // async fn read_meta_bytes<'a>(&'a self, path: &'a Path) -> Result<Vec<u8>, AssetReaderError> {
    //     unimplemented!()
    // }
}
