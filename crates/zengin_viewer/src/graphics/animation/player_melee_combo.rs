//! Skill driven 1H melee combos of the player.
//!
//! The talent overlay selected by [`PlayerMeleeCombo`]'s skill adds a
//! multi-segment `S_1HATTACK` animation. A press starts it, presses inside a
//! segment's combo window jump the playback to the next segment and a segment
//! that is not chained ends the swing early.

use std::sync::atomic::Ordering;

use bevy::prelude::*;
use leafwing_input_manager::prelude::ActionState;

use crate::gameplay::objects::{NpcModel, PlayerCharacter};
use crate::graphics::AnimationGraph;
use crate::input::{GameplayAction, GameplayInputMarker, WorldInputViews};
use crate::perf_debug;
use crate::zengin::{
    CachedAssetHandles, NpcMobUse, NpcTalentOverlay, NpcWeaponMode, Selection, WeaponMode,
};

use super::animation_triggers::{AnimationTriggers, AttackTimeline, animation_ident_from_path};
use super::auto_anim_sequencer::{
    AutoAnimSequencer, DesiredAnimation, MeleeAttackKind, is_melee_attack_animation,
    queue_desired_now,
};
use super::components::AnimationState;
use super::controller::AnimationController;
use super::locomotion::TIME_SCALE;

/// Animation of the skill dependent 1H combo.
const ONE_HAND_COMBO_ANIMATION: &str = "S_1HATTACK";

/// Segment of the player's current 1H attack combo, `0` when no combo runs.
#[derive(Component, Default, Debug)]
pub(crate) struct PlayerMeleeCombo {
    combo_len: usize,
    timeline: Option<AttackTimeline>,
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn player_melee_combo(
    mut commands: Commands,
    action_state: Single<&ActionState<GameplayAction>, With<GameplayInputMarker>>,
    views: WorldInputViews,
    graph: Option<Res<AnimationGraph>>,
    triggers: Res<AnimationTriggers>,
    asset_server: Res<AssetServer>,
    selection: Res<Selection>,
    transforms: Query<&GlobalTransform>,
    mut handles: ResMut<CachedAssetHandles>,
    mut players: Query<
        (
            Entity,
            &mut AnimationController,
            &AnimationState,
            Option<&mut PlayerMeleeCombo>,
            &mut AutoAnimSequencer,
            &NpcModel,
            &GlobalTransform,
            Option<&NpcWeaponMode>,
            Option<&NpcTalentOverlay>,
            Option<&NpcMobUse>,
        ),
        With<PlayerCharacter>,
    >,
) {
    let _perf = perf_debug!();
    let Some(graph) = graph else {
        return;
    };
    let Ok((
        entity,
        mut controller,
        state,
        combo,
        mut sequencer,
        model,
        player_tr,
        weapon_mode,
        talent_overlay,
        mob_use,
    )) = players.single_mut()
    else {
        return;
    };
    let Some(mut combo) = combo else {
        commands
            .entity(entity)
            .try_insert(PlayerMeleeCombo::default());
        return;
    };

    if !views.enabled() || mob_use.is_some() {
        combo.combo_len = 0;
        combo.timeline = None;
        return;
    }
    let mode = weapon_mode.map_or(WeaponMode::Fists, |mode| mode.0);
    let current = controller.current().clone();
    let is_melee_attack = is_melee_attack_animation(&current);
    let combo_ident = current
        .path()
        .and_then(|path| animation_ident_from_path(path.path()))
        .filter(|(_scheme, animation)| *animation == ONE_HAND_COMBO_ANIMATION);
    if mode != WeaponMode::Melee {
        combo.combo_len = 0;
        combo.timeline = None;
        return;
    }

    let pressed = action_state.just_pressed(&GameplayAction::Interact)
        || action_state.just_pressed(&GameplayAction::DealDamage);
    // Holding the attack key keeps the player swinging: a finished combo
    // starts the next one, while chaining inside it still needs a fresh press.
    let held = action_state.pressed(&GameplayAction::Interact)
        || action_state.pressed(&GameplayAction::DealDamage);

    let Some((scheme, animation)) = combo_ident else {
        combo.combo_len = 0;
        combo.timeline = None;
        if !held || is_melee_attack {
            return;
        }
        if queue_desired_now(
            &graph,
            &mut handles,
            &asset_server,
            &mut controller,
            model,
            weapon_mode,
            talent_overlay,
            DesiredAnimation::MeleeAttack(MeleeAttackKind::Combo),
        ) {
            face_target(&mut sequencer, player_tr, &selection, &transforms);
            debug!("Player 1H combo started");
        }
        return;
    };

    face_target(&mut sequencer, player_tr, &selection, &transforms);

    if combo.timeline.is_none() {
        let Some(timeline) = triggers.timeline(scheme, animation) else {
            return;
        };
        combo.timeline = Some(timeline.clone());
    }
    let progress = state.time_us.load(Ordering::Acquire) as f32 / TIME_SCALE * state.fps;
    let step = combo.timeline.as_ref().map_or(ComboStep::Wait, |timeline| {
        combo_step(progress, combo.combo_len, pressed, timeline)
    });
    match step {
        ComboStep::Chain { frame, next_len } => {
            seek_frame(state, frame);
            combo.combo_len = next_len;
            debug!("Player 1H combo chained to segment {next_len}");
        }
        ComboStep::End => {
            let desired = sequencer.desired.unwrap_or(DesiredAnimation::Stand);
            queue_desired_now(
                &graph,
                &mut handles,
                &asset_server,
                &mut controller,
                model,
                weapon_mode,
                talent_overlay,
                desired,
            );
            combo.combo_len = 0;
            combo.timeline = None;
        }
        ComboStep::Wait => {}
    }
}

/// One step of the combo state machine, decided from the last playback frame's
/// progress.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ComboStep {
    /// A press inside the window advances to the next segment.
    Chain { frame: u32, next_len: usize },
    /// The current segment played out and the swing ends.
    End,
    /// Keep playing the current segment.
    Wait,
}

fn combo_step(
    progress: f32,
    combo_len: usize,
    pressed: bool,
    timeline: &AttackTimeline,
) -> ComboStep {
    if pressed
        && timeline.can_chain(combo_len)
        && let Some((start, end)) = timeline.window(combo_len)
        && progress > start as f32
        && progress <= end as f32
    {
        return ComboStep::Chain {
            frame: timeline.hit_end[combo_len],
            next_len: combo_len + 1,
        };
    }
    match timeline.hit_end.get(combo_len) {
        Some(end) if progress >= *end as f32 => ComboStep::End,
        Some(_) => ComboStep::Wait,
        None => ComboStep::End,
    }
}

fn seek_frame(state: &AnimationState, frame: u32) {
    if state.fps <= 0.0 {
        return;
    }
    let frame = frame.min(state.frames_num.saturating_sub(1) as u32);
    let time_us = (frame as f32 / state.fps * TIME_SCALE) as u32;
    state.time_us.store(time_us, Ordering::Release);
}

fn face_target(
    sequencer: &mut AutoAnimSequencer,
    player_tr: &GlobalTransform,
    selection: &Selection,
    transforms: &Query<&GlobalTransform>,
) {
    let Some(target) = selection
        .target
        .and_then(|target| transforms.get(target).ok())
    else {
        return;
    };
    let delta = target.translation() - player_tr.translation();
    let direction = Vec3::new(delta.x, 0.0, delta.z);
    if direction.length_squared() > 0.0001 {
        sequencer.direction = Some(direction.normalize());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn timeline() -> AttackTimeline {
        AttackTimeline {
            hit_end: vec![33, 68, 103, 119],
            windows: vec![(12, 36), (47, 71), (82, 106)],
        }
    }

    #[test]
    fn press_inside_the_window_chains_to_the_next_segment() {
        let step = combo_step(20.0, 0, true, &timeline());

        assert_eq!(
            step,
            ComboStep::Chain {
                frame: 33,
                next_len: 1
            }
        );
    }

    #[test]
    fn press_before_or_after_the_window_does_not_chain() {
        assert_eq!(combo_step(5.0, 0, true, &timeline()), ComboStep::Wait);
        assert_eq!(combo_step(40.0, 0, true, &timeline()), ComboStep::End);
    }

    #[test]
    fn segment_end_without_a_press_ends_the_swing() {
        assert_eq!(combo_step(33.0, 0, false, &timeline()), ComboStep::End);
        assert_eq!(combo_step(40.0, 0, false, &timeline()), ComboStep::End);
    }

    #[test]
    fn the_last_segment_cannot_chain_further() {
        assert_eq!(combo_step(100.0, 3, true, &timeline()), ComboStep::Wait);
        assert_eq!(combo_step(119.0, 3, true, &timeline()), ComboStep::End);
    }
}
