
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct ZkInternal_Vfs {
    _unused: [u8; 0],
}
pub type ZkVfs = ZkInternal_Vfs;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct ZkInternal_VfsNode {
    _unused: [u8; 0],
}
pub type ZkVfsNode = ZkInternal_VfsNode;
#[repr(u32)]
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub enum ZkVfsOverwriteBehavior {
    ZkVfsOverwriteBehavior_NONE = 0,
    ZkVfsOverwriteBehavior_ALL = 1,
    ZkVfsOverwriteBehavior_NEWER = 2,
    ZkVfsOverwriteBehavior_OLDER = 3,
}
pub type ZkVfsNodeEnumerator = ::std::option::Option<
    unsafe extern "C" fn(ctx: *mut ::std::os::raw::c_void, node: *const ZkVfsNode) -> ZkBool,
>;
unsafe extern "C" {
    pub fn ZkVfs_new() -> *mut ZkVfs;


    pub fn ZkVfs_del(slf: *mut ZkVfs);


    pub fn ZkVfs_save(slf: *const ZkVfs, path: ZkString, version: ZkGameVersion, unix_t: time_t);


    pub fn ZkVfs_getRoot(slf: *const ZkVfs) -> *const ZkVfsNode;


    pub fn ZkVfs_mkdir(slf: *mut ZkVfs, path: ZkString) -> *mut ZkVfsNode;


    pub fn ZkVfs_remove(slf: *mut ZkVfs, path: ZkString) -> ZkBool;


    pub fn ZkVfs_mount(
        slf: *mut ZkVfs,
        node: *mut ZkVfsNode,
        parent: ZkString,
        overwrite: ZkVfsOverwriteBehavior,
    );


    pub fn ZkVfs_mountHost(
        slf: *mut ZkVfs,
        path: ZkString,
        parent: ZkString,
        overwrite: ZkVfsOverwriteBehavior,
    );


    pub fn ZkVfs_mountDisk(slf: *mut ZkVfs, buf: *mut ZkRead, overwrite: ZkVfsOverwriteBehavior);


    pub fn ZkVfs_mountDiskHost(slf: *mut ZkVfs, path: ZkString, overwrite: ZkVfsOverwriteBehavior);


    pub fn ZkVfs_resolvePath(slf: *mut ZkVfs, path: ZkString) -> *mut ZkVfsNode;


    pub fn ZkVfs_findNode(slf: *mut ZkVfs, name: ZkString) -> *mut ZkVfsNode;


    pub fn ZkVfsNode_newFile(
        name: ZkString,
        buf: *const ZkByte,
        size: ZkSize,
        ts: time_t,
    ) -> *mut ZkVfsNode;


    pub fn ZkVfsNode_newDir(name: ZkString, ts: time_t) -> *mut ZkVfsNode;


    pub fn ZkVfsNode_del(slf: *mut ZkVfsNode);


    pub fn ZkVfsNode_isFile(slf: *const ZkVfsNode) -> ZkBool;


    pub fn ZkVfsNode_isDir(slf: *const ZkVfsNode) -> ZkBool;


    pub fn ZkVfsNode_getTime(slf: *const ZkVfsNode) -> time_t;


    pub fn ZkVfsNode_getName(slf: *const ZkVfsNode) -> ZkString;


    pub fn ZkVfsNode_getChild(slf: *mut ZkVfsNode, name: ZkString) -> *mut ZkVfsNode;


    pub fn ZkVfsNode_create(slf: *mut ZkVfsNode, node: *mut ZkVfsNode) -> *mut ZkVfsNode;


    pub fn ZkVfsNode_remove(slf: *mut ZkVfsNode, name: ZkString) -> ZkBool;


    pub fn ZkVfsNode_open(slf: *const ZkVfsNode) -> *mut ZkRead;


    pub fn ZkVfsNode_enumerateChildren(
        slf: *const ZkVfsNode,
        callback: ZkVfsNodeEnumerator,
        ctx: *mut ::std::os::raw::c_void,
    );
}