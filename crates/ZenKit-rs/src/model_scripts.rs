use crate::conversions::*;
use crate::stream::Read;
use crate::vfs::Vfs;
use ZenKitCAPI_sys::*;
use std::marker::PhantomData;
use std::ptr::NonNull;

#[derive(Debug)]
pub struct ModelScript<'a> {
    ptr: NonNull<ZkModelScript>,
    _marker: PhantomData<&'a ZkModelScript>,
    owned: bool,
}

impl ModelScript<'_> {
    pub unsafe fn from_raw_ptr(ptr: *mut ZkModelScript, owned: bool) -> Option<Self> {
        NonNull::new(ptr).map(|ptr| Self {
            ptr,
            owned,
            _marker: PhantomData,
        })
    }

    pub fn load(read: &Read) -> Option<Self> {
        unsafe {
            let ptr = ZkModelScript_load(read.as_raw());
            return Self::from_raw_ptr(ptr, true);
        };
    }

    pub fn load_path(path: &str) -> Option<Self> {
        let cstr = str_to_zk(path);
        unsafe {
            let ptr = ZkModelScript_loadPath(cstr.as_ptr());
            return Self::from_raw_ptr(ptr, true);
        };
    }

    pub fn load_vfs(vfs: &Vfs, name: &str) -> Option<Self> {
        let cstr = str_to_zk(name);
        unsafe {
            let ptr = ZkModelScript_loadVfs(vfs.get_raw(), cstr.as_ptr());
            return Self::from_raw_ptr(ptr, true);
        };
    }

    pub fn skeleton_name(&self) -> String {
        unsafe { ZkModelScript_getSkeletonName(self.ptr.as_ptr()).to_string() }
    }
    pub fn skeleton_mesh_disabled(&self) -> bool {
        unsafe { ZkModelScript_getSkeletonMeshDisabled(self.ptr.as_ptr()).to_bool() }
    }

    pub fn mesh_count(&self) -> u64 {
        unsafe { ZkModelScript_getMeshCount(self.ptr.as_ptr()) }
    }
    pub fn mesh(&self, index: u64) -> String {
        unsafe { ZkModelScript_getMesh(self.ptr.as_ptr(), index).to_string() }
    }
    pub fn meshes(&self) -> Vec<String> {
        let count = self.mesh_count();
        (0..count).map(|i| self.mesh(i)).collect()
    }

    pub fn disabled_animations_count(&self) -> u64 {
        unsafe { ZkModelScript_getDisabledAnimationsCount(self.ptr.as_ptr()) }
    }
    pub fn disabled_animation(&self, index: u64) -> String {
        unsafe { ZkModelScript_getDisabledAnimation(self.ptr.as_ptr(), index).to_string() }
    }
    pub fn disabled_animations(&self) -> Vec<String> {
        let count = self.disabled_animations_count();
        (0..count).map(|i| self.disabled_animation(i)).collect()
    }

    pub fn animation_combine_count(&self) -> u64 {
        unsafe { ZkModelScript_getAnimationCombineCount(self.ptr.as_ptr()) }
    }
    // unsafe extern "C" {
    //     pub fn ZkModelScript_getAnimationCombine(
    //         slf: *const ZkModelScript,
    //         i: ZkSize,
    //     ) -> *const ZkAnimationCombine;
    // }

    pub fn animation_blend_count(&self) -> u64 {
        unsafe { ZkModelScript_getAnimationBlendCount(self.ptr.as_ptr()) }
    }
    // unsafe extern "C" {
    //     pub fn ZkModelScript_getAnimationBlend(
    //         slf: *const ZkModelScript,
    //         i: ZkSize,
    //     ) -> *const ZkAnimationBlend;
    // }

    pub fn animation_alias_count(&self) -> u64 {
        unsafe { ZkModelScript_getAnimationAliasCount(self.ptr.as_ptr()) }
    }
    // unsafe extern "C" {
    //     pub fn ZkModelScript_getAnimationAlias(
    //         slf: *const ZkModelScript,
    //         i: ZkSize,
    //     ) -> *const ZkAnimationAlias;
    // }

    pub fn model_tag_count(&self) -> u64 {
        unsafe { ZkModelScript_getModelTagCount(self.ptr.as_ptr()) }
    }
    pub fn model_tag(&self, index: u64) -> String {
        unsafe { ZkModelScript_getModelTag(self.ptr.as_ptr(), index).to_string() }
    }
    pub fn tags(&self) -> Vec<String> {
        let count = self.model_tag_count();
        (0..count).map(|i| self.model_tag(i)).collect()
    }

    pub fn animation_count(&self) -> u64 {
        unsafe { ZkModelScript_getAnimationCount(self.ptr.as_ptr()) }
    }
    pub fn animation(&self, index: u64) -> Animation<'_> {
        unsafe {
            let ptr = ZkModelScript_getAnimation(self.ptr.as_ptr(), index);
            Animation::from_raw_ptr(ptr.cast_mut()).unwrap()
        }
    }
    pub fn animations(&self) -> Vec<Animation<'_>> {
        let count = self.animation_count();
        (0..count).map(|i| self.animation(i)).collect()
    }
}

impl Drop for ModelScript<'_> {
    fn drop(&mut self) {
        if self.owned {
            unsafe { ZkModelScript_del(self.ptr.as_ptr()) };
        }
    }
}

#[derive(Debug)]
pub struct Animation<'a> {
    ptr: NonNull<ZkAnimation>,
    _marker: PhantomData<&'a ZkModelScript>,
}

impl Animation<'_> {
    pub unsafe fn from_raw_ptr(ptr: *mut ZkAnimation) -> Option<Self> {
        NonNull::new(ptr).map(|ptr| Self {
            ptr,
            _marker: PhantomData,
        })
    }

    pub fn flags(&self) -> ZkAnimationFlag {
        unsafe { ZkAnimation_getFlags(self.ptr.as_ptr()) }
    }
    pub fn direction(&self) -> ZkAnimationDirection {
        unsafe { ZkAnimation_getDirection(self.ptr.as_ptr()) }
    }

    pub fn blend_in(&self) -> f32 {
        unsafe { ZkAnimation_getBlendIn(self.ptr.as_ptr()) }
    }
    pub fn blend_out(&self) -> f32 {
        unsafe { ZkAnimation_getBlendOut(self.ptr.as_ptr()) }
    }
    pub fn fps(&self) -> f32 {
        unsafe { ZkAnimation_getFps(self.ptr.as_ptr()) }
    }
    pub fn speed(&self) -> f32 {
        unsafe { ZkAnimation_getSpeed(self.ptr.as_ptr()) }
    }
    pub fn collision_volume_scale(&self) -> f32 {
        unsafe { ZkAnimation_getCollisionVolumeScale(self.ptr.as_ptr()) }
    }

    pub fn layer(&self) -> u32 {
        unsafe { ZkAnimation_getLayer(self.ptr.as_ptr()) }
    }

    pub fn first_frame(&self) -> i32 {
        unsafe { ZkAnimation_getFirstFrame(self.ptr.as_ptr()) }
    }
    pub fn last_frame(&self) -> i32 {
        unsafe { ZkAnimation_getLastFrame(self.ptr.as_ptr()) }
    }

    pub fn name(&self) -> String {
        unsafe { ZkAnimation_getName(self.ptr.as_ptr()).to_string() }
    }
    pub fn next(&self) -> String {
        unsafe { ZkAnimation_getNext(self.ptr.as_ptr()).to_string() }
    }
    pub fn model(&self) -> String {
        unsafe { ZkAnimation_getModel(self.ptr.as_ptr()).to_string() }
    }

    pub fn event_tag_count(&self) -> u64 {
        unsafe { ZkAnimation_getEventTagCount(self.ptr.as_ptr()) }
    }
    pub fn particle_effect_count(&self) -> u64 {
        unsafe { ZkAnimation_getParticleEffectCount(self.ptr.as_ptr()) }
    }
    pub fn particle_effect_stop_count(&self) -> u64 {
        unsafe { ZkAnimation_getParticleEffectStopCount(self.ptr.as_ptr()) }
    }
    pub fn sound_effect_count(&self) -> u64 {
        unsafe { ZkAnimation_getSoundEffectCount(self.ptr.as_ptr()) }
    }
    pub fn sound_effect_ground_count(&self) -> u64 {
        unsafe { ZkAnimation_getSoundEffectGroundCount(self.ptr.as_ptr()) }
    }
    pub fn morph_animation_count(&self) -> u64 {
        unsafe { ZkAnimation_getMorphAnimationCount(self.ptr.as_ptr()) }
    }
    pub fn camera_tremor_count(&self) -> u64 {
        unsafe { ZkAnimation_getCameraTremorCount(self.ptr.as_ptr()) }
    }

    // pub fn ZkAnimation_getEventTag(slf: *const ZkAnimation, i: ZkSize) -> *const ZkEventTag;

    // pub fn ZkAnimation_getParticleEffect(
    //     slf: *const ZkAnimation,
    //     i: ZkSize,
    // ) -> *const ZkEventParticleEffect;

    // pub fn ZkAnimation_getParticleEffectStop(
    //     slf: *const ZkAnimation,
    //     i: ZkSize,
    // ) -> *const ZkEventParticleEffectStop;

    // pub fn ZkAnimation_getSoundEffect(
    //     slf: *const ZkAnimation,
    //     i: ZkSize,
    // ) -> *const ZkEventSoundEffect;

    // pub fn ZkAnimation_getSoundEffectGround(
    //     slf: *const ZkAnimation,
    //     i: ZkSize,
    // ) -> *const ZkEventSoundEffectGround;

    // pub fn ZkAnimation_getMorphAnimation(
    //     slf: *const ZkAnimation,
    //     i: ZkSize,
    // ) -> *const ZkEventMorphAnimation;

    // pub fn ZkAnimation_getCameraTremor(
    //     slf: *const ZkAnimation,
    //     i: ZkSize,
    // ) -> *const ZkEventCameraTremor;
}
