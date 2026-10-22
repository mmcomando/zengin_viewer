use crate::conversions::*;
use crate::misc::Mat3;
use crate::misc::Vec3;
use crate::misc::*;
use crate::stream::Read;
use ZenKitCAPI_sys::*;
use std::marker::PhantomData;
use std::ptr::NonNull;

pub struct VirtualObject<'a> {
    ptr: NonNull<ZkVirtualObject>,
    _marker: PhantomData<&'a ZkVirtualObject>,
    owned: bool,
}

impl VirtualObject<'_> {
    pub unsafe fn from_raw_ptr(ptr: *mut ZkVirtualObject, owned: bool) -> Option<Self> {
        NonNull::new(ptr).map(|ptr| Self {
            ptr,
            owned,
            _marker: PhantomData,
        })
    }

    pub fn load(reader: &Read, version: GameVersion) -> Option<Self> {
        unsafe {
            let ptr = ZkVirtualObject_load(reader.as_raw(), version);
            return Self::from_raw_ptr(ptr, false);
        };
    }

    pub fn load_path(path: &str, version: GameVersion) -> Option<Self> {
        let cstr = str_to_zk(path);
        unsafe {
            let ptr = ZkVirtualObject_loadPath(cstr.as_ptr(), version);
            return Self::from_raw_ptr(ptr, false);
        };
    }

    pub fn show_visual(&self) -> bool {
        unsafe { ZkVirtualObject_getShowVisual(self.ptr.as_ptr()).to_bool() }
    }
    pub fn cd_static(&self) -> bool {
        unsafe { ZkVirtualObject_getCdStatic(self.ptr.as_ptr()).to_bool() }
    }
    pub fn cd_dynamic(&self) -> bool {
        unsafe { ZkVirtualObject_getCdDynamic(self.ptr.as_ptr()).to_bool() }
    }
    pub fn vob_static(&self) -> bool {
        unsafe { ZkVirtualObject_getVobStatic(self.ptr.as_ptr()).to_bool() }
    }
    pub fn physics_enabled(&self) -> bool {
        unsafe { ZkVirtualObject_getPhysicsEnabled(self.ptr.as_ptr()).to_bool() }
    }
    pub fn ambient(&self) -> bool {
        unsafe { ZkVirtualObject_getAmbient(self.ptr.as_ptr()).to_bool() }
    }

    pub fn anim_strength(&self) -> f32 {
        unsafe { ZkVirtualObject_getAnimStrength(self.ptr.as_ptr()) }
    }
    pub fn far_clip_scale(&self) -> f32 {
        unsafe { ZkVirtualObject_getFarClipScale(self.ptr.as_ptr()) }
    }
    pub fn next_on_timer(&self) -> f32 {
        unsafe { ZkVirtualObject_getNextOnTimer(self.ptr.as_ptr()) }
    }

    pub fn preset_name(&self) -> String {
        unsafe { ZkVirtualObject_getPresetName(self.ptr.as_ptr()).to_string() }
    }
    pub fn name(&self) -> String {
        unsafe { ZkVirtualObject_getName(self.ptr.as_ptr()).to_string() }
    }

    pub fn get_type(&self) -> VobType {
        unsafe { ZkVirtualObject_getType(self.ptr.as_ptr()) }
    }
    pub fn id(&self) -> u32 {
        unsafe { ZkVirtualObject_getId(self.ptr.as_ptr()) }
    }

    pub fn bbox(&self) -> AxisAlignedBoundingBox {
        unsafe { ZkVirtualObject_getBbox(self.ptr.as_ptr()).to_own() }
    }
    pub fn position(&self) -> Vec3 {
        unsafe { ZkVirtualObject_getPosition(self.ptr.as_ptr()).to_own() }
    }
    pub fn rotation(&self) -> Mat3 {
        unsafe { ZkVirtualObject_getRotation(self.ptr.as_ptr()).to_own() }
    }

    pub fn visual(&self) -> Visual<'_> {
        unsafe {
            let ptr = ZkVirtualObject_getVisual(self.ptr.as_ptr());
            return Visual::from_raw_ptr(ptr, false).unwrap();
        }
    }

    pub fn child_count(&self) -> u64 {
        unsafe { ZkVirtualObject_getChildCount(self.ptr.as_ptr()) }
    }

    pub fn child(&self, index: u64) -> VirtualObject<'_> {
        unsafe {
            let ptr = ZkVirtualObject_getChild(self.ptr.as_ptr(), index);
            return VirtualObject::from_raw_ptr(ptr.cast(), false).unwrap();
        }
    }

    pub fn children(&self) -> Vec<VirtualObject<'_>> {
        let count = self.child_count();
        (0..count).map(|i| self.child(i)).collect()
    }
}

impl Drop for VirtualObject<'_> {
    fn drop(&mut self) {
        if self.owned {
            unsafe { ZkVirtualObject_del(self.ptr.as_ptr()) };
        }
    }
}

// pub fn ZkVirtualObject_getSpriteCameraFacingMode(
// pub fn ZkVirtualObject_getDynamicShadows(slf: *const ZkVirtualObject) -> ZkShadowType;
// pub fn ZkVirtualObject_getAnimMode(slf: *const ZkVirtualObject) -> ZkAnimationType;
// pub fn ZkVirtualObject_getBias(slf: *const ZkVirtualObject) -> i32;

// pub fn ZkVirtualObject_getSleepMode(slf: *const ZkVirtualObject) -> ZkByte;

// pub fn ZkVirtualObject_getAi(slf: *const ZkVirtualObject) -> *mut ZkAi;
// pub fn ZkVirtualObject_getEventManager(slf: *const ZkVirtualObject) -> *mut ZkEventManager;
//
//
//
// pub fn ZkVirtualObject_new(type_: ZkVobType) -> *mut ZkVirtualObject;
// pub fn ZkVirtualObject_setBbox(slf: *mut ZkVirtualObject, bbox: ZkAxisAlignedBoundingBox);
// pub fn ZkVirtualObject_setPosition(slf: *mut ZkVirtualObject, position: ZkVec3f);
// pub fn ZkVirtualObject_setRotation(slf: *mut ZkVirtualObject, rotation: ZkMat3x3);
// pub fn ZkVirtualObject_setShowVisual(slf: *mut ZkVirtualObject, showVisual: ZkBool);
// pub fn ZkVirtualObject_setSpriteCameraFacingMode(
// pub fn ZkVirtualObject_setCdStatic(slf: *mut ZkVirtualObject, cdStatic: ZkBool);
// pub fn ZkVirtualObject_setCdDynamic(slf: *mut ZkVirtualObject, cdDynamic: ZkBool);
// pub fn ZkVirtualObject_setVobStatic(slf: *mut ZkVirtualObject, vobStatic: ZkBool);
// pub fn ZkVirtualObject_setDynamicShadows(
// pub fn ZkVirtualObject_setPhysicsEnabled(slf: *mut ZkVirtualObject, physicsEnabled: ZkBool);
// pub fn ZkVirtualObject_setAnimMode(slf: *mut ZkVirtualObject, animMode: ZkAnimationType);
// pub fn ZkVirtualObject_setBias(slf: *mut ZkVirtualObject, bias: i32);
// pub fn ZkVirtualObject_setAmbient(slf: *mut ZkVirtualObject, ambient: ZkBool);
// pub fn ZkVirtualObject_setAnimStrength(slf: *mut ZkVirtualObject, animStrength: f32);
// pub fn ZkVirtualObject_setFarClipScale(slf: *mut ZkVirtualObject, farClipScale: f32);
// pub fn ZkVirtualObject_setPresetName(slf: *mut ZkVirtualObject, presetName: ZkString);
// pub fn ZkVirtualObject_setName(slf: *mut ZkVirtualObject, name: ZkString);
// pub fn ZkVirtualObject_setVisual(slf: *mut ZkVirtualObject, visual: *mut ZkVisual);
// pub fn ZkVirtualObject_setSleepMode(slf: *mut ZkVirtualObject, sleepMode: ZkByte);
// pub fn ZkVirtualObject_setNextOnTimer(slf: *mut ZkVirtualObject, nextOnTimer: f32);
// pub fn ZkVirtualObject_setAi(slf: *mut ZkVirtualObject, ai: *mut ZkAi);
// pub fn ZkVirtualObject_setEventManager(slf: *mut ZkVirtualObject, em: *mut ZkEventManager);
// pub fn ZkVirtualObject_addChild(slf: *mut ZkVirtualObject, object: *mut ZkVirtualObject);
// pub fn ZkVirtualObject_removeChild(slf: *mut ZkVirtualObject, i: ZkSize);
// pub fn ZkVirtualObject_removeChildren(

pub struct Visual<'a> {
    ptr: NonNull<ZkVisual>,
    _marker: PhantomData<&'a ZkVisual>,
    _owned: bool,
}

impl Visual<'_> {
    pub unsafe fn from_raw_ptr(ptr: *mut ZkVisual, owned: bool) -> Option<Self> {
        NonNull::new(ptr).map(|ptr| Self {
            ptr,
            _owned: owned,
            _marker: PhantomData,
        })
    }

    pub fn name(&self) -> String {
        unsafe { ZkVisual_getName(self.ptr.as_ptr()).to_string() }
    }
    pub fn get_type(&self) -> VisualType {
        unsafe { ZkVisual_getType(self.ptr.as_ptr()) }
    }
}

// ZKC_API ZkVisual* ZkVisual_new(ZkVisualType type);
// ZKC_API void ZkVisual_del(ZkVisual* slf);
// ZKC_API void ZkVisual_setName(ZkVisual* slf, ZkString name);

pub struct Npc<'a> {
    ptr: NonNull<ZkNpc>,
    _marker: PhantomData<&'a ZkNpc>,
    _owned: bool,
}

impl Npc<'_> {
    pub unsafe fn from_raw_ptr(ptr: *mut ZkNpc, owned: bool) -> Option<Self> {
        NonNull::new(ptr).map(|ptr| Self {
            ptr,
            _owned: owned,
            _marker: PhantomData,
        })
    }

    pub fn npc_instance(&self) -> String {
        unsafe { ZkNpc_getNpcInstance(self.ptr.as_ptr()).to_string() }
    }
    pub fn start_ai_state(&self) -> String {
        unsafe { ZkNpc_getStartAiState(self.ptr.as_ptr()).to_string() }
    }
    pub fn script_waypoint(&self) -> String {
        unsafe { ZkNpc_getScriptWaypoint(self.ptr.as_ptr()).to_string() }
    }
    pub fn current_state_name(&self) -> String {
        unsafe { ZkNpc_getCurrentStateName(self.ptr.as_ptr()).to_string() }
    }
    pub fn next_state_name(&self) -> String {
        unsafe { ZkNpc_getNextStateName(self.ptr.as_ptr()).to_string() }
    }
    pub fn current_routine(&self) -> String {
        unsafe { ZkNpc_getCurrentRoutine(self.ptr.as_ptr()).to_string() }
    }
    pub fn overlay(&self, index: u64) -> String {
        unsafe { ZkNpc_getOverlay(self.ptr.as_ptr(), index).to_string() }
    }
    pub fn packed(&self, index: u64) -> String {
        unsafe { ZkNpc_getPacked(self.ptr.as_ptr(), index).to_string() }
    }

    pub fn wounded(&self) -> bool {
        unsafe { ZkNpc_getWounded(self.ptr.as_ptr()).to_bool() }
    }
    pub fn mad(&self) -> bool {
        unsafe { ZkNpc_getMad(self.ptr.as_ptr()).to_bool() }
    }
    pub fn player(&self) -> bool {
        unsafe { ZkNpc_getPlayer(self.ptr.as_ptr()).to_bool() }
    }
    pub fn move_lock(&self) -> bool {
        unsafe { ZkNpc_getMoveLock(self.ptr.as_ptr()).to_bool() }
    }
    pub fn current_state_valid(&self) -> bool {
        unsafe { ZkNpc_getCurrentStateValid(self.ptr.as_ptr()).to_bool() }
    }
    pub fn current_state_is_routine(&self) -> bool {
        unsafe { ZkNpc_getCurrentStateIsRoutine(self.ptr.as_ptr()).to_bool() }
    }
    pub fn next_state_valid(&self) -> bool {
        unsafe { ZkNpc_getNextStateValid(self.ptr.as_ptr()).to_bool() }
    }
    pub fn next_state_is_routine(&self) -> bool {
        unsafe { ZkNpc_getNextStateIsRoutine(self.ptr.as_ptr()).to_bool() }
    }
    pub fn has_routine(&self) -> bool {
        unsafe { ZkNpc_getHasRoutine(self.ptr.as_ptr()).to_bool() }
    }
    pub fn routine_changed(&self) -> bool {
        unsafe { ZkNpc_getRoutineChanged(self.ptr.as_ptr()).to_bool() }
    }
    pub fn routine_overlay(&self) -> bool {
        unsafe { ZkNpc_getRoutineOverlay(self.ptr.as_ptr()).to_bool() }
    }

    pub fn weaponmode_routine(&self) -> bool {
        unsafe { ZkNpc_getWeaponmodeRoutine(self.ptr.as_ptr()).to_bool() }
    }
    pub fn start_new_routine(&self) -> bool {
        unsafe { ZkNpc_getStartNewRoutine(self.ptr.as_ptr()).to_bool() }
    }
    pub fn respawn(&self) -> bool {
        unsafe { ZkNpc_getRespawn(self.ptr.as_ptr()).to_bool() }
    }

    pub fn model_scale(&self) -> Vec3 {
        unsafe { ZkNpc_getModelScale(self.ptr.as_ptr()).to_own() }
    }
    pub fn ai_state_pos(&self) -> Vec3 {
        unsafe { ZkNpc_getAiStatePos(self.ptr.as_ptr()).to_own() }
    }
}

// pub fn ZkNpc_getFlags(slf: *const ZkNpc) -> ::std::os::raw::c_int;
// pub fn ZkNpc_getGuild(slf: *const ZkNpc) -> ::std::os::raw::c_int;
// pub fn ZkNpc_getGuildTrue(slf: *const ZkNpc) -> ::std::os::raw::c_int;
// pub fn ZkNpc_getLevel(slf: *const ZkNpc) -> ::std::os::raw::c_int;
// pub fn ZkNpc_getXp(slf: *const ZkNpc) -> ::std::os::raw::c_int;
// pub fn ZkNpc_getXpNextLevel(slf: *const ZkNpc) -> ::std::os::raw::c_int;
// pub fn ZkNpc_getLp(slf: *const ZkNpc) -> ::std::os::raw::c_int;
// pub fn ZkNpc_getFightTactic(slf: *const ZkNpc) -> ::std::os::raw::c_int;
// pub fn ZkNpc_getFightMode(slf: *const ZkNpc) -> ::std::os::raw::c_int;
// pub fn ZkNpc_getMadTime(slf: *const ZkNpc) -> ::std::os::raw::c_int;
// pub fn ZkNpc_getAttitude(slf: *const ZkNpc) -> ::std::os::raw::c_int;
// pub fn ZkNpc_getAttitudeTemp(slf: *const ZkNpc) -> ::std::os::raw::c_int;
// pub fn ZkNpc_getNameNr(slf: *const ZkNpc) -> ::std::os::raw::c_int;
// pub fn ZkNpc_getCurrentStateIndex(slf: *const ZkNpc) -> ::std::os::raw::c_int;
// pub fn ZkNpc_getNextStateIndex(slf: *const ZkNpc) -> ::std::os::raw::c_int;
// pub fn ZkNpc_getLastAiState(slf: *const ZkNpc) -> ::std::os::raw::c_int;
// pub fn ZkNpc_getRoutineOverlayCount(slf: *const ZkNpc) -> ::std::os::raw::c_int;
// pub fn ZkNpc_getWalkmodeRoutine(slf: *const ZkNpc) -> ::std::os::raw::c_int;
// pub fn ZkNpc_getAiStateDriven(slf: *const ZkNpc) -> ::std::os::raw::c_int;
// pub fn ZkNpc_getRespawnTime(slf: *const ZkNpc) -> ::std::os::raw::c_int;
// pub fn ZkNpc_getBsInterruptableOverride(slf: *const ZkNpc) -> ::std::os::raw::c_int;
// pub fn ZkNpc_getNpcType(slf: *const ZkNpc) -> ::std::os::raw::c_int;
// pub fn ZkNpc_getSpellMana(slf: *const ZkNpc) -> ::std::os::raw::c_int;
// pub fn ZkNpc_getProtection(slf: *const ZkNpc, i: ZkSize) -> ::std::os::raw::c_int;
// pub fn ZkNpc_getAttribute(slf: *const ZkNpc, i: ZkSize) -> ::std::os::raw::c_int;
// pub fn ZkNpc_getHitChance(slf: *const ZkNpc, i: ZkSize) -> ::std::os::raw::c_int;
// pub fn ZkNpc_getMission(slf: *const ZkNpc, i: ZkSize) -> ::std::os::raw::c_int;
// pub fn ZkNpc_getAiVars(slf: *const ZkNpc, len: *mut ZkSize) -> *const ::std::os::raw::c_int;

// pub fn ZkNpc_getModelFatness(slf: *const ZkNpc) -> f32;
// pub fn ZkNpc_getCarryVob(slf: *const ZkNpc) -> *mut ZkVirtualObject;
// pub fn ZkNpc_getEnemy(slf: *const ZkNpc) -> *mut ZkVirtualObject;
// pub fn ZkNpc_getOverlayCount(slf: *const ZkNpc) -> ZkSize;
// pub fn ZkNpc_getTalentCount(slf: *const ZkNpc) -> ZkSize;
// pub fn ZkNpc_getTalent(slf: *const ZkNpc, i: ZkSize) -> *mut ZkNpcTalent;
// pub fn ZkNpc_getItemCount(slf: *const ZkNpc) -> ZkSize;
// pub fn ZkNpc_getItem(slf: *const ZkNpc, i: ZkSize) -> *mut ZkItem;
// pub fn ZkNpc_getSlotCount(slf: *const ZkNpc) -> ZkSize;
// pub fn ZkNpc_getSlot(slf: *const ZkNpc, i: ZkSize) -> *mut ZkNpcSlot;
// pub fn ZkNpc_getNewsCount(slf: *const ZkNpc) -> ZkSize;
// pub fn ZkNpc_getNews(slf: *const ZkNpc, i: ZkSize) -> *mut ZkNpcNews;
//
//
// pub fn ZkNpc_setNpcInstance(slf: *mut ZkNpc, npcInstance: ZkString);
// pub fn ZkNpc_setModelScale(slf: *mut ZkNpc, modelScale: ZkVec3f);
// pub fn ZkNpc_setModelFatness(slf: *mut ZkNpc, modelFatness: f32);
// pub fn ZkNpc_setFlags(slf: *mut ZkNpc, flags: ::std::os::raw::c_int);
// pub fn ZkNpc_setGuild(slf: *mut ZkNpc, guild: ::std::os::raw::c_int);
// pub fn ZkNpc_setGuildTrue(slf: *mut ZkNpc, guildTrue: ::std::os::raw::c_int);
// pub fn ZkNpc_setLevel(slf: *mut ZkNpc, level: ::std::os::raw::c_int);
// pub fn ZkNpc_setXp(slf: *mut ZkNpc, xp: ::std::os::raw::c_int);
// pub fn ZkNpc_setXpNextLevel(slf: *mut ZkNpc, xpNextLevel: ::std::os::raw::c_int);
// pub fn ZkNpc_setLp(slf: *mut ZkNpc, lp: ::std::os::raw::c_int);
// pub fn ZkNpc_setFightTactic(slf: *mut ZkNpc, fightTactic: ::std::os::raw::c_int);
// pub fn ZkNpc_setFightMode(slf: *mut ZkNpc, fightMode: ::std::os::raw::c_int);
// pub fn ZkNpc_setWounded(slf: *mut ZkNpc, wounded: ZkBool);
// pub fn ZkNpc_setMad(slf: *mut ZkNpc, mad: ZkBool);
// pub fn ZkNpc_setMadTime(slf: *mut ZkNpc, madTime: ::std::os::raw::c_int);
// pub fn ZkNpc_setPlayer(slf: *mut ZkNpc, player: ZkBool);
// pub fn ZkNpc_setStartAiState(slf: *mut ZkNpc, startAiState: ZkString);
// pub fn ZkNpc_setScriptWaypoint(slf: *mut ZkNpc, scriptWaypoint: ZkString);
// pub fn ZkNpc_setAttitude(slf: *mut ZkNpc, attitude: ::std::os::raw::c_int);
// pub fn ZkNpc_setAttitudeTemp(slf: *mut ZkNpc, attitudeTemp: ::std::os::raw::c_int);
// pub fn ZkNpc_setNameNr(slf: *mut ZkNpc, nameNr: ::std::os::raw::c_int);
// pub fn ZkNpc_setMoveLock(slf: *mut ZkNpc, moveLock: ZkBool);
// pub fn ZkNpc_setCurrentStateValid(slf: *mut ZkNpc, currentStateValid: ZkBool);
// pub fn ZkNpc_setCurrentStateName(slf: *mut ZkNpc, currentStateName: ZkString);
// pub fn ZkNpc_setCurrentStateIndex(slf: *mut ZkNpc, currentStateIndex: ::std::os::raw::c_int);
// pub fn ZkNpc_setCurrentStateIsRoutine(slf: *mut ZkNpc, currentStateIsRoutine: ZkBool);
// pub fn ZkNpc_setNextStateValid(slf: *mut ZkNpc, nextStateValid: ZkBool);
// pub fn ZkNpc_setNextStateName(slf: *mut ZkNpc, nextStateName: ZkString);
// pub fn ZkNpc_setNextStateIndex(slf: *mut ZkNpc, nextStateIndex: ::std::os::raw::c_int);
// pub fn ZkNpc_setNextStateIsRoutine(slf: *mut ZkNpc, nextStateIsRoutine: ZkBool);
// pub fn ZkNpc_setLastAiState(slf: *mut ZkNpc, lastAiState: ::std::os::raw::c_int);
// pub fn ZkNpc_setHasRoutine(slf: *mut ZkNpc, hasRoutine: ZkBool);
// pub fn ZkNpc_setRoutineChanged(slf: *mut ZkNpc, routineChanged: ZkBool);
// pub fn ZkNpc_setRoutineOverlay(slf: *mut ZkNpc, routineOverlay: ZkBool);
// pub fn ZkNpc_setRoutineOverlayCount(
// pub fn ZkNpc_setWalkmodeRoutine(slf: *mut ZkNpc, walkmodeRoutine: ::std::os::raw::c_int);
// pub fn ZkNpc_setWeaponmodeRoutine(slf: *mut ZkNpc, weaponmodeRoutine: ZkBool);
// pub fn ZkNpc_setStartNewRoutine(slf: *mut ZkNpc, startNewRoutine: ZkBool);
// pub fn ZkNpc_setAiStateDriven(slf: *mut ZkNpc, aiStateDriven: ::std::os::raw::c_int);
// pub fn ZkNpc_setAiStatePos(slf: *mut ZkNpc, aiStatePos: ZkVec3f);
// pub fn ZkNpc_setCurrentRoutine(slf: *mut ZkNpc, currentRoutine: ZkString);
// pub fn ZkNpc_setRespawn(slf: *mut ZkNpc, respawn: ZkBool);
// pub fn ZkNpc_setRespawnTime(slf: *mut ZkNpc, respawnTime: ::std::os::raw::c_int);
// pub fn ZkNpc_setBsInterruptableOverride(
// pub fn ZkNpc_setNpcType(slf: *mut ZkNpc, npcType: ::std::os::raw::c_int);
// pub fn ZkNpc_setSpellMana(slf: *mut ZkNpc, spellMana: ::std::os::raw::c_int);
// pub fn ZkNpc_setCarryVob(slf: *mut ZkNpc, carryVob: *mut ZkVirtualObject);
// pub fn ZkNpc_setEnemy(slf: *mut ZkNpc, enemy: *mut ZkVirtualObject);
// pub fn ZkNpc_clearOverlays(slf: *mut ZkNpc);
// pub fn ZkNpc_removeOverlay(slf: *mut ZkNpc, i: ZkSize);
// pub fn ZkNpc_setOverlay(slf: *mut ZkNpc, i: ZkSize, overlay: ZkString);
// pub fn ZkNpc_addOverlay(slf: *mut ZkNpc, overlay: ZkString);
// pub fn ZkNpc_clearTalents(slf: *mut ZkNpc);
// pub fn ZkNpc_removeTalent(slf: *mut ZkNpc, i: ZkSize);
// pub fn ZkNpc_setTalent(slf: *mut ZkNpc, i: ZkSize, talent: *mut ZkNpcTalent);
// pub fn ZkNpc_addTalent(slf: *mut ZkNpc, talent: *mut ZkNpcTalent);
// pub fn ZkNpc_clearItems(slf: *mut ZkNpc);
// pub fn ZkNpc_removeItem(slf: *mut ZkNpc, i: ZkSize);
// pub fn ZkNpc_setItem(slf: *mut ZkNpc, i: ZkSize, item: *mut ZkItem);
// pub fn ZkNpc_addItem(slf: *mut ZkNpc, item: *mut ZkItem);
// pub fn ZkNpc_clearSlots(slf: *mut ZkNpc);
// pub fn ZkNpc_removeSlot(slf: *mut ZkNpc, i: ZkSize);
// pub fn ZkNpc_addSlot(slf: *mut ZkNpc) -> *mut ZkNpcSlot;
// pub fn ZkNpc_clearNews(slf: *mut ZkNpc);
// pub fn ZkNpc_removeNews(slf: *mut ZkNpc, i: ZkSize);
// pub fn ZkNpc_addNews(slf: *mut ZkNpc) -> *mut ZkNpcNews;
// pub fn ZkNpc_setProtection(slf: *mut ZkNpc, i: ZkSize, v: ::std::os::raw::c_int);
// pub fn ZkNpc_setAttribute(slf: *mut ZkNpc, i: ZkSize, v: ::std::os::raw::c_int);
// pub fn ZkNpc_setHitChance(slf: *mut ZkNpc, i: ZkSize, v: ::std::os::raw::c_int);
// pub fn ZkNpc_setMission(slf: *mut ZkNpc, i: ZkSize, v: ::std::os::raw::c_int);
// pub fn ZkNpc_setAiVars(slf: *const ZkNpc, vars: *const ::std::os::raw::c_int, len: ZkSize);
// pub fn ZkNpc_setPacked(slf: *const ZkNpc, i: ZkSize, v: ZkString);
