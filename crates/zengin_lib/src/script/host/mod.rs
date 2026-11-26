use super::bindings::mmcomando;
use super::journal::{JournalAction, JournalChange, JournalSection, JournalStatus};
use super::state::{
    AlignToWpChange, AnimationChange, AttackChange, BS_STAND, ClearAiQueueChange, DialogChoice,
    DialogLine, GotoNpcChange, ITEM_MAINFLAG_ARMOR, ImportCallState, InventoryGain,
    MobRoutineEntry, OverlayRequest, PassivePerception, ProcessInfosChange, RoutineEntry,
    RoutineExchange, SetGameTime, StandUpChange, StartStateChange, TurnToNpcChange,
    UseItemStateChange, UseMobChange, WaitChange, WeaponKind, WeaponMode, current_body_state,
    has_body_flag,
};
use super::string_value::StringCanon;
use super::symbol_id::InstanceId;
use crate::common::to_zengin_units;
use bevy::log::warn;
use bevy::math::Vec3;

/// `ATT_HOSTILE` script constant, the default guild attitude.
pub const ATT_HOSTILE: i32 = 0;
/// `ATT_NEUTRAL` script constant.
pub const ATT_NEUTRAL: i32 = 2;
/// `ATT_FRIENDLY` script constant.
pub const ATT_FRIENDLY: i32 = 3;

// Fully qualified because this macro expands in the `host` submodules, which
// do not import `warn_unimplemented`.
macro_rules! fake_impl {
    ($name:ident($($arg:ident),*)) => {
        fn $name(&mut self, $($arg: i32),*) {
            dev_util::warn_unimplemented!(stringify!($name));
        }
    };
    ($name:ident($($arg:ident),*) -> i32) => {
        fn $name(&mut self, $($arg: i32),*) -> i32 {
            dev_util::warn_unimplemented!(stringify!($name));
            0
        }
    };
    // String-returning externals must record an entry in the DYN_STRINGS
    // mirror even when stubbed, the mirror stays in sync with the guest ring
    // cursor only if every string-returning call is recorded
    ($name:ident($($arg:ident),*) -> Vec<u8>) => {
        fn $name(&mut self, $($arg: i32),*) -> Vec<u8> {
            dev_util::warn_unimplemented!(stringify!($name));
            self.record_dynamic_string(Vec::new());
            Vec::new()
        }
    };

}

pub mod ai;
pub mod apply;
pub mod doc;
pub mod game;
pub mod hlp;
pub mod info;
pub mod log;
pub mod mdl;
pub mod mis;
pub mod misc;
pub mod mob;
pub mod npc;
pub mod snd;
pub mod symbols;
pub mod ta;
pub mod wld;
