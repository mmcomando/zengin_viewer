use crate::conversions::*;
use crate::misc::*;
use crate::stream::Read;
use ZenKitCAPI_sys::*;
use std::ptr::NonNull;

pub struct Vfs {
    vfs: NonNull<ZkVfs>,
}

// Vfs internal structure are C++ types which should be safe to send between threads
unsafe impl Send for Vfs {}

#[derive(Debug)]
pub struct VfsNode<'a> {
    node: NonNull<ZkVfsNode>,
    _marker: std::marker::PhantomData<&'a ZkVfsNode>,
}

impl<'a> VfsNode<'a> {
    pub fn from_raw(node_raw: *mut ZkVfsNode) -> Option<Self> {
        let node = NonNull::new(node_raw)?;
        Some(Self {
            node,
            _marker: std::marker::PhantomData,
        })
    }

    pub fn is_file(&self) -> bool {
        let result = unsafe { ZkVfsNode_isFile(self.node.as_ptr()) };
        result.to_bool()
    }

    pub fn time(&self) -> FileTime {
        let t = unsafe { ZkVfsNode_getTime(self.node.as_ptr()) };
        FileTime::from(t)
    }

    pub fn name(&self) -> String {
        let ptr = unsafe { ZkVfsNode_getName(self.node.as_ptr()) };
        ptr.to_string()
    }

    pub fn get_child(&self, name: &str) -> Option<VfsNode<'a>> {
        let cstr = str_to_zk(name);
        let raw = unsafe { ZkVfsNode_getChild(self.node.as_ptr(), cstr.as_ptr()) };
        VfsNode::from_raw(raw)
    }

    pub fn remove_child(&self, name: &str) -> bool {
        let cstr = str_to_zk(name);
        let result = unsafe { ZkVfsNode_remove(self.node.as_ptr(), cstr.as_ptr()) };
        result.to_bool()
    }

    pub fn open(&self) -> Option<Read> {
        let raw = unsafe { ZkVfsNode_open(self.node.as_ptr()) };
        Read::from_raw(raw)
    }

    pub fn enumerate_children(&self) -> Vec<VfsNode<'a>> {
        let mut raw_nodes: Vec<NonNull<ZkVfsNode>> = Vec::new();

        unsafe {
            extern "C" fn trampoline(ctx: *mut std::ffi::c_void, node: *const ZkVfsNode) -> ZkBool {
                let vec = unsafe { &mut *ctx.cast::<Vec<NonNull<ZkVfsNode>>>() };

                if let Some(nn) = NonNull::new(node.cast_mut()) {
                    vec.push(nn);
                }
                0 // Continue iteration
            }
            ZkVfsNode_enumerateChildren(
                self.node.as_ptr(),
                Some(trampoline),
                std::ptr::from_mut(&mut raw_nodes).cast::<std::ffi::c_void>(),
            );
        }

        raw_nodes
            .into_iter()
            .map(|node| VfsNode {
                node,
                _marker: std::marker::PhantomData,
            })
            .collect()
    }
}

impl Vfs {
    pub fn new() -> Self {
        let vfs = unsafe { NonNull::new(ZkVfs_new()) }
            .expect("ZkVfs_new should always return non null value");
        Self { vfs }
    }

    pub fn get_raw(&self) -> *mut ZkVfs {
        return self.vfs.as_ptr();
    }

    pub fn save(&self, path: &str, version: GameVersion, unix_time: FileTime) {
        let cstr = str_to_zk(path);
        unsafe {
            ZkVfs_save(
                self.vfs.as_ptr(),
                cstr.as_ptr(),
                version,
                unix_time.as_number(),
            );
        }
    }

    pub fn get_root(&self) -> VfsNode<'_> {
        let node_raw = unsafe { ZkVfs_getRoot(self.vfs.as_ptr()) }.cast_mut();
        VfsNode::from_raw(node_raw).expect("Vfs should always contain root node")
    }

    pub fn mkdir<'a>(&'a self, path: &str) -> Option<VfsNode<'a>> {
        let cstr = str_to_zk(path);
        let node = unsafe { ZkVfs_mkdir(self.vfs.as_ptr(), cstr.as_ptr()) };
        VfsNode::from_raw(node)
    }

    pub fn remove(&self, path: &str) -> bool {
        let cstr = str_to_zk(path);
        let result = unsafe { ZkVfs_remove(self.vfs.as_ptr(), cstr.as_ptr()) };
        result.to_bool()
    }

    pub fn mount_host(&self, path: &str, parent: &str, overwrite: VfsOverwriteBehavior) {
        let path_cstr = str_to_zk(path);
        let parent_cstr = str_to_zk(parent);
        unsafe {
            ZkVfs_mountHost(
                self.vfs.as_ptr(),
                path_cstr.as_ptr(),
                parent_cstr.as_ptr(),
                overwrite,
            );
        };
    }

    pub fn mount_disk(&self, read: &Read, overwrite: VfsOverwriteBehavior) {
        unsafe { ZkVfs_mountDisk(self.vfs.as_ptr(), read.as_raw(), overwrite) };
    }

    pub fn mount_disk_host(&self, path: &str, overwrite: VfsOverwriteBehavior) {
        let cstr = str_to_zk(path);
        unsafe { ZkVfs_mountDiskHost(self.vfs.as_ptr(), cstr.as_ptr(), overwrite) };
    }

    pub fn resolve_path<'a>(&'a self, path: &str) -> Option<VfsNode<'a>> {
        let cstr = str_to_zk(path);
        let node = unsafe { ZkVfs_resolvePath(self.vfs.as_ptr(), cstr.as_ptr()) };
        VfsNode::from_raw(node)
    }

    pub fn find_node<'a>(&'a self, name: &str) -> Option<VfsNode<'a>> {
        let cstr = str_to_zk(name);
        let node = unsafe { ZkVfs_findNode(self.vfs.as_ptr(), cstr.as_ptr()) };
        VfsNode::from_raw(node)
    }
}

impl Drop for Vfs {
    fn drop(&mut self) {
        unsafe { ZkVfs_del(self.vfs.as_ptr()) };
    }
}

// pub struct OwnedVfsNode {
//     node: NonNull<ZkVfsNode>,
// }

// impl OwnedVfsNode {
//
// pub fn ZkVfsNode_newFile(
//        name: ZkString,
//        buf: *const ZkByte,
//        size: ZkSize,
//        ts: time_t,
//    ) -> *mut ZkVfsNode;
//
//     pub fn new_dir(name: &str, ts: FileTime) -> Option<Self> {
//         let cstr = str_to_zk(name);

//         let raw = unsafe { ZkVfsNode_newDir(cstr.as_ptr(), ts.0) };
//         let node = NonNull::new(raw)?;

//         Some(Self { node })
//     }

//     pub fn as_ptr(&self) -> *mut ZkVfsNode {
//         self.node.as_ptr()
//     }

//     /// Convert to borrowed view
//     pub fn as_borrowed<'a>(&'a self) -> VfsNode<'a> {
//         VfsNode {
//             node: self.node,
//             _marker: std::marker::PhantomData,
//         }
//     }
//
//

// }

// impl Drop for OwnedVfsNode {
//     fn drop(&mut self) {
//         unsafe { ZkVfsNode_del(self.node.as_ptr()) };
//     }
// }

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use crate::testing_common::*;

//     #[test]
//     fn basic() {
//         let vfs = Vfs::new();
//         let root = vfs.get_root();
//         assert_eq!(root.name(), "/");
//     }

//     #[test]
//     fn find() {
//         let vfs = Vfs::new();
//         assert!(vfs.find_node("nothing").is_none());
//     }

//     #[test]
//     fn mkdir() {
//         let vfs = Vfs::new();
//         vfs.mkdir("/Test");
//         vfs.find_node("Test").expect("Should find node");
//     }

//     #[test]
//     fn mount_disk_host() {
//         let vfs = Vfs::new();
//         vfs.mount_disk_host(&worlds_dir(), VfsOverwriteBehavior::ALL);

//         vfs.resolve_path("/_WORK/DATA/WORLDS/NEWWORLD/NEWWORLD.ZEN")
//             .expect("Should find node");
//         vfs.find_node("DATA").expect("Should find node");
//     }

//     #[test]
//     fn list_nodes() {
//         let vfs = Vfs::new();
//         vfs.mount_disk_host(&worlds_dir(), VfsOverwriteBehavior::ALL);

//         let node = vfs
//             .resolve_path("/_WORK/DATA/WORLDS")
//             .expect("Should find node");
//         let nodes = node.enumerate_children();
//         assert_eq!(nodes.len(), 6);
//     }
// }
