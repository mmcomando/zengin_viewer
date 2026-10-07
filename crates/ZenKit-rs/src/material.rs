use crate::conversions::*;
use crate::misc::*;
use ZenKitCAPI_sys::*;
use std::marker::PhantomData;
use std::ptr::NonNull;

// TODO owned type
// pub fn ZkMaterial_load(buf: *mut ZkRead) -> *mut ZkMaterial;
// pub fn ZkMaterial_loadPath(path: ZkString) -> *mut ZkMaterial;
// pub fn ZkMaterial_del(slf: *mut ZkMaterial);

pub struct Material<'a> {
    ptr: NonNull<ZkMaterial>,
    _marker: PhantomData<&'a ZkMaterial>,
}

impl Material<'_> {
    pub fn from_raw(ptr: *mut ZkMaterial) -> Option<Self> {
        let ptr = NonNull::new(ptr)?;
        Some(Self {
            ptr,
            _marker: std::marker::PhantomData,
        })
    }

    pub fn name(&self) -> String {
        unsafe {
            let ptr = ZkMaterial_getName(self.ptr.as_ptr());
            ptr.to_string()
        }
    }
    pub fn object_detail(&self) -> String {
        unsafe {
            let ptr = ZkMaterial_getDetailObject(self.ptr.as_ptr());
            ptr.to_string()
        }
    }

    pub fn color(&self) -> U8Vec4 {
        unsafe { ZkMaterial_getColor(self.ptr.as_ptr()).to_own() }
    }
    pub fn texture(&self) -> String {
        unsafe {
            let ptr = ZkMaterial_getTexture(self.ptr.as_ptr());
            ptr.to_string()
        }
    }

    pub fn disable_collision(&self) -> bool {
        unsafe { ZkMaterial_getDisableCollision(self.ptr.as_ptr()).to_bool() }
    }
    pub fn disable_lightmap(&self) -> bool {
        unsafe { ZkMaterial_getDisableLightmap(self.ptr.as_ptr()).to_bool() }
    }
    pub fn dont_collapse(&self) -> bool {
        unsafe { ZkMaterial_getDontCollapse(self.ptr.as_ptr()).to_bool() }
    }
    pub fn force_occluder(&self) -> bool {
        unsafe { ZkMaterial_getForceOccluder(self.ptr.as_ptr()).to_bool() }
    }
    pub fn environment_mapping(&self) -> bool {
        unsafe { ZkMaterial_getEnvironmentMapping(self.ptr.as_ptr()).to_bool() }
    }
    pub fn ignore_sun(&self) -> bool {
        unsafe { ZkMaterial_getIgnoreSun(self.ptr.as_ptr()).to_bool() }
    }

    pub fn smooth_angle(&self) -> f32 {
        unsafe { ZkMaterial_getSmoothAngle(self.ptr.as_ptr()) }
    }
    pub fn texture_animation_fps(&self) -> f32 {
        unsafe { ZkMaterial_getTextureAnimationFps(self.ptr.as_ptr()) }
    }
    pub fn detail_object_scale(&self) -> f32 {
        unsafe { ZkMaterial_getDetailObjectScale(self.ptr.as_ptr()) }
    }
    pub fn environment_mapping_strength(&self) -> f32 {
        unsafe { ZkMaterial_getEnvironmentMappingStrength(self.ptr.as_ptr()) }
    }
    pub fn wave_amplitude(&self) -> f32 {
        unsafe { ZkMaterial_getWaveAmplitude(self.ptr.as_ptr()) }
    }
    pub fn wave_grid_size(&self) -> f32 {
        unsafe { ZkMaterial_getWaveGridSize(self.ptr.as_ptr()) }
    }

    pub fn texture_scale(&self) -> Vec2 {
        unsafe { ZkMaterial_getTextureScale(self.ptr.as_ptr()).to_own() }
    }
    pub fn texture_animation_mapping_direction(&self) -> Vec2 {
        unsafe { ZkMaterial_getTextureAnimationMappingDirection(self.ptr.as_ptr()).to_own() }
    }
    pub fn default_mapping(&self) -> Vec2 {
        unsafe { ZkMaterial_getDefaultMapping(self.ptr.as_ptr()).to_own() }
    }

    pub fn wave_mode(&self) -> WaveMode {
        unsafe { ZkMaterial_getWaveMode(self.ptr.as_ptr()) }
    }
    pub fn wave_speed(&self) -> WaveSpeed {
        unsafe { ZkMaterial_getWaveSpeed(self.ptr.as_ptr()) }
    }
    pub fn alpha_function(&self) -> AlphaFunction {
        unsafe { ZkMaterial_getAlphaFunction(self.ptr.as_ptr()) }
    }
    pub fn group(&self) -> MaterialGroup {
        unsafe { ZkMaterial_getGroup(self.ptr.as_ptr()) }
    }
    pub fn texture_animation_mapping(&self) -> AnimationMapping {
        unsafe { ZkMaterial_getTextureAnimationMapping(self.ptr.as_ptr()) }
    }
}
