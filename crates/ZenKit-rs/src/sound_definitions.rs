//! Sound definitions from the compiled `SFX.DAT` script: sound instance name
//! (e.g. `WAR_AMBIENT`) mapped to its wav file name.

use std::collections::HashMap;
use std::ffi::c_void;
use std::ptr::NonNull;

use ZenKitCAPI_sys::*;

use crate::conversions::{ZkStringToString, str_to_zk};
use crate::vfs::Vfs;

pub struct SoundDefinitions {
    files: HashMap<String, String>,
}

impl SoundDefinitions {
    pub fn load_path(path: &str) -> Option<Self> {
        let cstr = str_to_zk(path);
        let vm = unsafe { ZkDaedalusVm_loadPath(cstr.as_ptr(), 0) };
        Self::from_vm(vm)
    }

    pub fn load_vfs(vfs: &Vfs, name: &str) -> Option<Self> {
        let cstr = str_to_zk(name);
        let vm = unsafe { ZkDaedalusVm_loadVfs(vfs.get_raw(), cstr.as_ptr(), 0) };
        Self::from_vm(vm)
    }

    /// Wav file name of the sound definition, None when the script has no
    /// instance with that name.
    pub fn file(&self, name: &str) -> Option<&str> {
        self.files.get(name).map(String::as_str)
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    fn from_vm(raw: *mut ZkDaedalusVm) -> Option<Self> {
        let vm = NonNull::new(raw)?;
        let files = Self::read_files(vm);
        unsafe { ZkDaedalusVm_del(vm.as_ptr()) };
        Some(Self { files })
    }

    fn read_files(vm: NonNull<ZkDaedalusVm>) -> HashMap<String, String> {
        // The C API exposes symbol access only on the script, and the VM
        // wrapper stores the (single, offset zero) script base as its first
        // member.
        let script = vm.as_ptr().cast::<ZkDaedalusScript>();

        let member_name = str_to_zk("C_SFX.FILE");
        let member = unsafe { ZkDaedalusScript_getSymbolByName(script, member_name.as_ptr()) };
        let Some(member) = NonNull::new(member) else {
            return HashMap::new();
        };

        let mut files = HashMap::new();
        let mut ctx = CollectCtx {
            vm: vm.as_ptr(),
            file_member: member.as_ptr(),
            files: &mut files,
        };
        let class_name = str_to_zk("C_SFX");
        unsafe {
            ZkDaedalusScript_enumerateInstanceSymbols(
                script,
                class_name.as_ptr(),
                Some(collect_sound),
                std::ptr::from_mut(&mut ctx).cast::<c_void>(),
            );
        }

        files
    }
}

struct CollectCtx<'a> {
    vm: *mut ZkDaedalusVm,
    file_member: *mut ZkDaedalusSymbol,
    files: &'a mut HashMap<String, String>,
}

unsafe extern "C" fn collect_sound(ctx: *mut c_void, symbol: *mut ZkDaedalusSymbol) -> ZkBool {
    let ctx = unsafe { &mut *ctx.cast::<CollectCtx<'_>>() };
    let Some(symbol) = NonNull::new(symbol) else {
        return 0;
    };

    let instance = unsafe {
        ZkDaedalusVm_initInstance(ctx.vm, symbol.as_ptr(), ZkDaedalusInstanceType::SoundEffect)
    };
    let Some(instance) = NonNull::new(instance) else {
        return 0;
    };

    let name = unsafe { ZkDaedalusSymbol_getName(symbol.as_ptr()) }.to_string();
    let file =
        unsafe { ZkDaedalusSymbol_getString(ctx.file_member, 0, instance.as_ptr()) }.to_string();
    unsafe { ZkDaedalusInstance_release(instance.as_ptr()) };

    ctx.files.insert(name, file);
    0
}
