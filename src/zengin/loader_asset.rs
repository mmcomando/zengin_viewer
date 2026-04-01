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
use std::{
    path::{Path, PathBuf},
    pin::Pin,
    task::{Context, Poll},
};
use zen_kit_rs::{misc::VfsOverwriteBehavior, vfs::Vfs};

use crate::zengin::common::gothic2_dir;

pub fn get_gothic_assert_bytes(path: &str) -> Option<Vec<u8>> {
    let vfs = Vfs::new();
    let textures_path = format!("{}/Data/Textures.vdf", gothic2_dir());
    vfs.mount_disk_host(&textures_path, VfsOverwriteBehavior::ALL);

    // println!("get_gothic_assert_bytes({path})");
    let path = format!("/{}", &path);
    let node = vfs.resolve_path(&path)?;
    let read_obj = node.open().unwrap();

    Some(read_obj.bytes())
}

pub fn create_gothic_asset_loader() -> AssetSourceBuilder {
    AssetSourceBuilder::new(move || Box::new(MyAssetReader {}))
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
            *dst = *src;
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
        Err(ReaderNotSeekableError)
    }
}

struct MyAssetReader;
impl AssetReader for MyAssetReader {
    async fn read<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        let path_str = path.as_os_str().to_str().unwrap();
        let Some(bytes) = get_gothic_assert_bytes(path_str) else {
            return Err(AssetReaderError::NotFound(path.to_path_buf()));
        };
        let val: Box<dyn Reader> = Box::new(MyAsyncReader::new(bytes));
        Ok(val)
    }

    async fn read_meta<'a>(
        &'a self,
        _path: &'a Path,
    ) -> Result<impl Reader + 'a, AssetReaderError> {
        // No metadata for assets from gothic
        Err::<Box<dyn Reader>, AssetReaderError>(AssetReaderError::NotFound(PathBuf::new()))
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
}
