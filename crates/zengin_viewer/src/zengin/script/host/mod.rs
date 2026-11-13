use super::bindings::mmcomando;
use super::journal::{JournalAction, JournalChange, JournalSection, JournalStatus};
use super::state::{
    AlignToWpChange, AnimationChange, AttackChange, BS_STAND, ClearAiQueueChange, DialogChoice,
    DialogLine, ITEM_MAINFLAG_ARMOR, ImportCallState, InventoryGain, MobRoutineEntry,
    OverlayRequest, RoutineEntry, RoutineExchange, SpawnItem, SpawnNpc, StartStateChange,
    UseMobChange, WeaponKind, WeaponMode, current_body_state, has_body_flag,
};
use super::string_value::StringCanon;
use super::symbol_id::InstanceId;
use crate::warn_unimplemented;
use crate::zengin::common::to_zengin_units;
use bevy::log::warn;
use bevy::math::Vec3;

macro_rules! fake_impl {
    ($name:ident($($arg:ident),*)) => {
        fn $name(&mut self, $($arg: i32),*) {
            $crate::warn_unimplemented!(stringify!($name));
        }
    };
    ($name:ident($($arg:ident),*) -> i32) => {
        fn $name(&mut self, $($arg: i32),*) -> i32 {
            $crate::warn_unimplemented!(stringify!($name));
            0
        }
    };
    // String-returning externals must record an entry in the DYN_STRINGS
    // mirror even when stubbed, the mirror stays in sync with the guest ring
    // cursor only if every string-returning call is recorded
    ($name:ident($($arg:ident),*) -> Vec<u8>) => {
        fn $name(&mut self, $($arg: i32),*) -> Vec<u8> {
            $crate::warn_unimplemented!(stringify!($name));
            self.record_dynamic_string(Vec::new());
            Vec::new()
        }
    };

}

mod ai;
mod apply;
mod doc;
mod game;
mod hlp;
mod info;
mod log;
mod mdl;
mod mis;
mod misc;
mod mob;
mod npc;
mod snd;
mod symbols;
mod ta;
mod wld;
