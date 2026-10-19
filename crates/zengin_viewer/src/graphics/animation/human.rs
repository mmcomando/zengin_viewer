//! Human animation state machine data.
//!
//! The enums describe the coarse animation states a human can be in; the
//! [`HumanAnimationSequence::animation_name`] mapping resolves them to the
//! scheme animation names for the v1 animation set (standing idle, the fists
//! run loop and the bow aiming overlay). Weapon specific locomotion sets and
//! walk animations are not wired yet.
//!
//! The sequencer component and its systems are added in a later phase; this
//! module only carries the state vocabulary and the animation names the
//! `HumanAnimationLibrary` resource preloads.

// The sequencer component and systems consuming these types come in a later
// phase; until then the API is intentionally unused.
#[allow(dead_code)]
/// Top level animation sequence of a human.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HumanAnimationSequence {
    Stand,
    WalkSequence(WalkSequence),
    RunSequence(RunSequence),
    FireBowSequence(FireBowSequence),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WalkSequence {
    Start,
    Loop,
    End,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RunSequence {
    Start,
    Loop,
    End,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FireBowSequence {
    Start,
    Loop,
    End,
}

impl HumanAnimationSequence {
    /// Animation name of the sequence state in the v1 animation set.
    ///
    /// Locomotion start/stop live inside the combined `S_FISTRUNL` frames
    /// (its first frames are the start, the tail the stop), so all
    /// `WalkSequence`/`RunSequence` states map to it until the playback
    /// supports combined animation loop ranges.
    pub fn animation_name(self) -> &'static str {
        match self {
            Self::Stand => "S_RUN",
            Self::WalkSequence(_) | Self::RunSequence(_) => "S_FISTRUNL",
            // The aim/fire hold `S_BOWSHOOT` is a 1 frame pose; the aim-in
            // transition loops instead until the release animation is known.
            Self::FireBowSequence(FireBowSequence::End) => "T_BOWAIM_2_BOWRUN",
            Self::FireBowSequence(_) => "T_BOWRUN_2_BOWAIM",
        }
    }
}

/// Every animation the human sequencer can reference in the v1 set.
pub const HUMAN_ANIMATIONS: [&str; 4] = [
    "S_RUN",
    "S_FISTRUNL",
    "T_BOWRUN_2_BOWAIM",
    "T_BOWAIM_2_BOWRUN",
];
