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
    pub fn animation_alias(&self, index: u64) -> Option<AnimationAlias<'_>> {
        unsafe {
            let ptr = ZkModelScript_getAnimationAlias(self.ptr.as_ptr(), index);
            AnimationAlias::from_raw_ptr(ptr)
        }
    }
    pub fn animation_aliases(&self) -> Vec<AnimationAlias<'_>> {
        (0..self.animation_alias_count())
            .map_while(|i| self.animation_alias(i))
            .collect()
    }

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

    // pub fn ZkAnimation_getParticleEffect(
    //     slf: *const ZkAnimation,
    //     i: ZkSize,
    // ) -> *const ZkEventParticleEffect;

    pub fn event_tag(&self, index: u64) -> Option<EventTag<'_>> {
        unsafe { EventTag::from_raw_ptr(ZkAnimation_getEventTag(self.ptr.as_ptr(), index)) }
    }

    pub fn event_tags(&self) -> Vec<EventTag<'_>> {
        (0..self.event_tag_count())
            .map_while(|i| self.event_tag(i))
            .collect()
    }

    pub fn sound_effect(&self, index: u64) -> Option<SoundEffect<'_>> {
        unsafe { SoundEffect::from_raw_ptr(ZkAnimation_getSoundEffect(self.ptr.as_ptr(), index)) }
    }

    pub fn sound_effects(&self) -> Vec<SoundEffect<'_>> {
        (0..self.sound_effect_count())
            .map_while(|i| self.sound_effect(i))
            .collect()
    }

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

/// Alias entry of a [`ModelScript`] — an animation name mapped onto another
/// animation (`alias (name = target ...)` in the model script).
#[derive(Debug)]
pub struct AnimationAlias<'a> {
    ptr: NonNull<ZkAnimationAlias>,
    _marker: PhantomData<&'a ZkModelScript>,
}

impl AnimationAlias<'_> {
    pub unsafe fn from_raw_ptr(ptr: *const ZkAnimationAlias) -> Option<Self> {
        NonNull::new(ptr.cast_mut()).map(|ptr| Self {
            ptr,
            _marker: PhantomData,
        })
    }

    /// Name the alias defines.
    pub fn name(&self) -> String {
        unsafe { ZkAnimationAlias_getName(self.ptr.as_ptr()).to_string() }
    }

    /// Animation the alias plays.
    pub fn alias(&self) -> String {
        unsafe { ZkAnimationAlias_getAlias(self.ptr.as_ptr()).to_string() }
    }

    /// Animation chained after the alias finishes.
    pub fn next(&self) -> String {
        unsafe { ZkAnimationAlias_getNext(self.ptr.as_ptr()).to_string() }
    }
}

/// Event tag of a [`Animation`] — a scripted event at a frame, e.g. placing or
/// removing an item in a slot, fight related frames.
#[derive(Debug)]
pub struct EventTag<'a> {
    ptr: NonNull<ZkEventTag>,
    _marker: PhantomData<&'a ZkAnimation>,
}

impl EventTag<'_> {
    pub unsafe fn from_raw_ptr(ptr: *const ZkEventTag) -> Option<Self> {
        NonNull::new(ptr.cast_mut()).map(|ptr| Self {
            ptr,
            _marker: PhantomData,
        })
    }

    /// First frame of the event, -1 when the event has an explicit frame list.
    pub fn frame(&self) -> i32 {
        unsafe { ZkEventTag_getFrame(self.ptr.as_ptr()) }
    }

    pub fn event_type(&self) -> ZkEventType {
        unsafe { ZkEventTag_getType(self.ptr.as_ptr()) }
    }

    /// Item instance name carried by item events (e.g. the arrow placed in hand).
    pub fn item(&self) -> String {
        unsafe { ZkEventTag_getItem(self.ptr.as_ptr()).to_string() }
    }

    /// Explicit frame list, used when [`Self::frame`] is -1 (e.g. hit limbs over
    /// multiple frames).
    pub fn frames(&self) -> Vec<i32> {
        unsafe {
            let mut count: ZkSize = 0;
            let ptr = ZkEventTag_getFrames(self.ptr.as_ptr(), &mut count);
            if ptr.is_null() {
                return Vec::new();
            }
            std::slice::from_raw_parts(ptr, count as usize).to_vec()
        }
    }
}

/// Sound effect event of an [`Animation`] — a sound played at a frame.
#[derive(Debug)]
pub struct SoundEffect<'a> {
    ptr: NonNull<ZkEventSoundEffect>,
    _marker: PhantomData<&'a ZkAnimation>,
}

impl SoundEffect<'_> {
    pub unsafe fn from_raw_ptr(ptr: *const ZkEventSoundEffect) -> Option<Self> {
        NonNull::new(ptr.cast_mut()).map(|ptr| Self {
            ptr,
            _marker: PhantomData,
        })
    }

    pub fn frame(&self) -> i32 {
        unsafe { ZkEventSoundEffect_getFrame(self.ptr.as_ptr()) }
    }

    /// Sound effect name, e.g. `S_BOW_SHOOT`.
    pub fn name(&self) -> String {
        unsafe { ZkEventSoundEffect_getName(self.ptr.as_ptr()).to_string() }
    }

    pub fn range(&self) -> f32 {
        unsafe { ZkEventSoundEffect_getRange(self.ptr.as_ptr()) }
    }
}
