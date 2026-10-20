//! Human animation state machine data.
//!
//! The enums describe the coarse animation states a human can be in; the
//! [`HumanAnimationSequence::animation_name`] mapping resolves them to the
//! scheme animation names for the v1 animation set (standing idle, the run
//! loop with its transitions and the bow aiming overlay). Walk animations and
//! weapon specific locomotion sets are not wired yet.
//!
//! The sequencer component and its systems live in the sibling sequencer
//! systems module; this module only carries the state vocabulary and the
//! animation names the `HumanAnimationLibrary` resource preloads.

use bevy::ecs::component::Component;
use bevy::math::Quat;
use bevy::math::Vec3;

/// Top level animation sequence of a human.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum HumanAnimationSequence {
    #[default]
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

/// Coarse family of a sequence state, used by the decision graph to detect
/// that the desired sequence type changed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SequenceFamily {
    Stand,
    /// Walk animations are not wired yet; the state exists for the run-only
    /// v1 plumbing.
    #[allow(dead_code)]
    /// Walk animations are not wired yet; run-only v1.
    #[allow(dead_code)]
    Walk,
    Run,
    FireBow,
}

/// Weapon stance influencing the standing idle of the sequencer.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ArmedState {
    #[default]
    Unarmed,
    Melee,
    Ranged,
}

/// Whether the entity walks or runs while moving.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum PreferedLocomotion {
    #[default]
    Run,
    /// Walk animations are not wired yet; run-only v1.
    #[allow(dead_code)]
    Walk,
}

/// What the entity wants to do. Movement intents carry the target facing
/// (`GoDir`), the waypoint name or the position to shoot at.
#[derive(Clone, PartialEq, Debug, Default)]
pub enum DesiredAction {
    #[default]
    Stand,
    RangedFireLocation(Vec3),
    GoDir(Quat),
    /// Waypoint the entity should walk to; resolved to a facing direction by
    /// the sequencer. Unused by the player input in v1, kept for the npc AI.
    #[allow(dead_code)]
    GoWaypoint(String),
}

impl HumanAnimationSequence {
    /// Coarse family of the sequence state.
    pub fn family(self) -> SequenceFamily {
        match self {
            Self::Stand => SequenceFamily::Stand,
            Self::WalkSequence(_) => SequenceFamily::Walk,
            Self::RunSequence(_) => SequenceFamily::Run,
            Self::FireBowSequence(_) => SequenceFamily::FireBow,
        }
    }

    /// Animation name of the sequence state in the v1 animation set.
    ///
    /// The standing idle depends on the weapon stance (`S_1H` and `S_BOW` are
    /// armed idles holding their single frame). Walk states map to the run
    /// animations until walk animations are wired.
    pub fn animation_name(self, armed: ArmedState) -> &'static str {
        match self {
            Self::Stand => match armed {
                ArmedState::Unarmed => "S_RUN",
                ArmedState::Melee => "S_1H",
                ArmedState::Ranged => "S_BOW",
            },
            Self::WalkSequence(_) | Self::RunSequence(_) => match self {
                Self::WalkSequence(WalkSequence::Start) | Self::RunSequence(RunSequence::Start) => {
                    "T_RUN_2_RUNL"
                }
                Self::WalkSequence(WalkSequence::End) | Self::RunSequence(RunSequence::End) => {
                    "T_RUNL_2_RUN"
                }
                _ => "S_RUNL",
            },
            Self::FireBowSequence(FireBowSequence::End) => "T_BOWAIM_2_BOWRUN",
            Self::FireBowSequence(FireBowSequence::Loop) => "S_BOWSHOOT",
            Self::FireBowSequence(FireBowSequence::Start) => "T_BOWRUN_2_BOWAIM",
        }
    }
}

/// Component driving the animation of a human via the sequencer state
/// machine. Lives on the animation model entity next to the
/// [`AnimationController`](super::AnimationController); the owning systems
/// compute `desired_state` from `desired_action` and walk the transition
/// graph on animation events.
#[derive(Component, Debug, Default)]
pub struct HumanAnimationSequencer {
    pub current_state: HumanAnimationSequence,
    pub desired_state: HumanAnimationSequence,
    pub prefered_locomotion: PreferedLocomotion,
    pub desired_action: DesiredAction,
    /// Seconds until the next arrow while holding the aim in
    /// `FireBowSequence::Loop`.
    pub shot_cooldown: f32,
}

/// Every animation the human sequencer can reference in the v1 set.
pub const HUMAN_ANIMATIONS: [&str; 9] = [
    "S_RUN",
    "S_1H",
    "S_BOW",
    "T_RUN_2_RUNL",
    "S_RUNL",
    "T_RUNL_2_RUN",
    "T_BOWRUN_2_BOWAIM",
    "S_BOWSHOOT",
    "T_BOWAIM_2_BOWRUN",
];
