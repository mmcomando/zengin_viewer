//! Music themes from the compiled `MUSIC.DAT` script.

use std::collections::HashMap;
use std::ffi::c_void;
use std::ptr::NonNull;

use ZenKitCAPI_sys::*;

use crate::conversions::{ZkStringToString, str_to_zk};
use crate::misc::{MusicTransitionEffect, MusicTransitionType};
use crate::vfs::Vfs;

/// One `C_MusicTheme` instance.
#[derive(Debug, Clone)]
pub struct MusicTheme {
    /// Segment file name, e.g. `gamestart.sgt`.
    pub file: String,
    pub volume: f32,
    pub looping: bool,
    pub reverb_mix: f32,
    pub reverb_time: f32,
    pub transition: MusicTransitionEffect,
    pub transition_type: MusicTransitionType,
}

/// Music theme instance name (case-insensitive) mapped to its data.
#[derive(Default)]
pub struct MusicDefinitions {
    themes: HashMap<String, MusicTheme>,
}

impl MusicDefinitions {
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

    /// Theme by instance name, case-insensitive.
    pub fn get(&self, name: &str) -> Option<&MusicTheme> {
        self.themes.get(&name.to_ascii_lowercase())
    }

    pub fn len(&self) -> usize {
        self.themes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.themes.is_empty()
    }

    fn from_vm(raw: *mut ZkDaedalusVm) -> Option<Self> {
        let vm = NonNull::new(raw)?;
        let themes = Self::read_themes(vm);
        unsafe { ZkDaedalusVm_del(vm.as_ptr()) };
        Some(Self { themes })
    }

    fn read_themes(vm: NonNull<ZkDaedalusVm>) -> HashMap<String, MusicTheme> {
        // The C API exposes symbol access only on the script, and the VM
        // wrapper stores the (single, offset zero) script base as its first
        // member.
        let script = vm.as_ptr().cast::<ZkDaedalusScript>();

        let mut themes = HashMap::new();
        let mut ctx = CollectCtx {
            vm: vm.as_ptr(),
            themes: &mut themes,
        };
        let class_name = str_to_zk("C_MusicTheme");
        unsafe {
            ZkDaedalusScript_enumerateInstanceSymbols(
                script,
                class_name.as_ptr(),
                Some(collect_theme),
                std::ptr::from_mut(&mut ctx).cast::<c_void>(),
            );
        }

        themes
    }
}

struct CollectCtx<'a> {
    vm: *mut ZkDaedalusVm,
    themes: &'a mut HashMap<String, MusicTheme>,
}

unsafe extern "C" fn collect_theme(ctx: *mut c_void, symbol: *mut ZkDaedalusSymbol) -> ZkBool {
    let ctx = unsafe { &mut *ctx.cast::<CollectCtx<'_>>() };
    let Some(symbol) = NonNull::new(symbol) else {
        return 0;
    };

    let instance = unsafe {
        ZkDaedalusVm_initInstance(ctx.vm, symbol.as_ptr(), ZkDaedalusInstanceType::MusicTheme)
    };
    let Some(instance) = NonNull::new(instance) else {
        return 0;
    };

    let name = unsafe { ZkDaedalusSymbol_getName(symbol.as_ptr()) }
        .to_string()
        .to_ascii_lowercase();
    let theme = unsafe { read_theme(instance) };
    unsafe { ZkDaedalusInstance_release(instance.as_ptr()) };

    ctx.themes.insert(name, theme);
    0
}

unsafe fn read_theme(instance: NonNull<ZkDaedalusInstance>) -> MusicTheme {
    let instance = instance.as_ptr().cast::<ZkMusicThemeInstance>();
    MusicTheme {
        file: unsafe { ZkMusicThemeInstance_getFile(instance) }.to_string(),
        volume: unsafe { ZkMusicThemeInstance_getVol(instance) },
        looping: unsafe { ZkMusicThemeInstance_getLoop(instance) } != 0,
        reverb_mix: unsafe { ZkMusicThemeInstance_getReverbmix(instance) },
        reverb_time: unsafe { ZkMusicThemeInstance_getReverbtime(instance) },
        transition: unsafe { ZkMusicThemeInstance_getTranstype(instance) },
        transition_type: unsafe { ZkMusicThemeInstance_getTranssubtype(instance) },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires a Gothic 2 install with MUSIC.DAT"]
    fn reads_menu_theme() {
        let _guard = crate::testing_common::zenkit_ffi();
        let path = format!(
            "{}/_work/Data/Scripts/_compiled/MUSIC.DAT",
            crate::testing_common::gothic2_dir()
        );
        let definitions = MusicDefinitions::load_path(&path).expect("MUSIC.DAT should load");

        assert!(definitions.len() > 50);
        let menu = definitions.get("SYS_MENU").expect("SYS_MENU");
        assert_eq!(menu.file.to_ascii_lowercase(), "gamestart.sgt");
        assert!(menu.looping);
        assert_eq!(menu.transition, MusicTransitionEffect::NONE);

        assert!(definitions.get("sys_menu").is_some());
        assert!(definitions.get("KHO_DAY_STD").is_some());
    }
}
