//! World movers (`zCMover`): visual vobs that travel along their keyframes when
//! a trigger event names them. A button (`oCMobInter.triggerTarget`) opens the
//! gate by emitting [`VobTrigger`] addressed to the mover's vob name.

use bevy::log::{debug, info};
use bevy::prelude::*;
use zen_kit_rs::misc::MoverBehavior;

use crate::game_state::AppState;
use dev_util::perf_debug;

pub struct MoversPlugin;

impl Plugin for MoversPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<VobTrigger>();
        app.init_resource::<PendingWorldStartTriggers>();
        app.add_systems(Update, (apply_vob_triggers, advance_movers));
        app.add_systems(OnEnter(AppState::InGame), fire_world_start_triggers);
    }
}

/// Trigger event addressed to every vob whose name matches `target`.
#[derive(Message, Debug, Clone)]
pub struct VobTrigger {
    pub target: String,
}

/// Targets of the world's `zCTriggerWorldStart` vobs. They are fired when the
/// world enters [`AppState::InGame`], so every mover they address exists.
#[derive(Resource, Default, Debug)]
pub(crate) struct PendingWorldStartTriggers(pub(crate) Vec<String>);

/// `zCMover` moving between its keyframes.
#[derive(Component, Debug, Clone, Reflect)]
pub struct GameMover {
    pub name: String,
    /// Entity transform the mover was spawned at (the ZEN vob pose).
    spawn: Transform,
    /// Keyframe poses relative to keyframe 0, so index 0 is the identity.
    keyframes: Vec<Transform>,
    /// Keyframe the mover last reached.
    current: usize,
    /// Keyframe it is travelling toward.
    target: usize,
    /// Progress of the current segment in `0..=1`.
    progress: f32,
    /// Movement speed in meters per second.
    speed: f32,
    /// `zTrigger.startEnabled`.
    pub enabled: bool,
    mode: MoverMode,
    /// `LOOP` mover cycling its keyframes until the world ends.
    looping: bool,
}

/// Mover behaviours the runtime acts on. The remaining behaviours keep the
/// pre-existing toggle reaction.
#[derive(Debug, Clone, Copy, Reflect)]
enum MoverMode {
    Toggle,
    Loop,
}

impl GameMover {
    /// `keyframes` are absolute world poses with keyframe 0 first, `speed` is in
    /// meters per second.
    pub fn new(
        name: String,
        keyframes: Vec<Transform>,
        spawn: Transform,
        speed: f32,
        enabled: bool,
        behavior: MoverBehavior,
    ) -> Self {
        let base = keyframes.first().copied().unwrap_or(spawn);
        let inverse_base = base.compute_affine().inverse();
        let keyframes = keyframes
            .iter()
            .map(|keyframe| {
                Transform::from_matrix(Mat4::from(inverse_base * keyframe.compute_affine()))
            })
            .collect();
        let mode = if behavior == MoverBehavior::LOOP {
            MoverMode::Loop
        } else {
            MoverMode::Toggle
        };
        Self {
            name,
            spawn,
            keyframes,
            current: 0,
            target: 0,
            progress: 0.0,
            speed,
            enabled,
            mode,
            looping: false,
        }
    }

    fn trigger(&mut self) {
        if !self.enabled || self.keyframes.len() < 2 {
            return;
        }
        match self.mode {
            MoverMode::Loop => {
                // A second trigger does not restart the loop; it is already
                // cycling.
                if !self.looping {
                    self.looping = true;
                    self.current = 0;
                    self.target = 1;
                    self.progress = 0.0;
                }
            }
            MoverMode::Toggle => self.toggle(),
        }
    }

    /// Toggles a two-state mover the way `zCMover` `TOGGLE` behaviour does:
    /// keyframe 0 is one end of the path, the last keyframe the other.
    fn toggle(&mut self) {
        if !self.enabled || self.keyframes.len() < 2 {
            return;
        }
        let last = self.keyframes.len() - 1;
        let heading = if self.current == self.target {
            self.current
        } else {
            self.target
        };
        self.target = if heading == 0 { last } else { 0 };
        self.progress = 0.0;
    }

    /// Places the mover on [`Self::target`] and queues the next segment: the
    /// following keyframe while looping, otherwise nothing.
    fn finish_segment(&mut self, transform: &mut Transform) {
        self.current = self.target;
        self.progress = 0.0;
        *transform = self.spawn.mul_transform(self.keyframes[self.current]);
        if self.looping {
            self.target = (self.current + 1) % self.keyframes.len();
        }
    }

    fn segment_duration(&self, from: &Transform, to: &Transform) -> f32 {
        let distance = from.translation.distance(to.translation);
        if distance > 1e-4 && self.speed > 0.0 {
            return distance / self.speed;
        }
        // Rotation-only movers are rare; a fixed angular speed keeps them
        // from snapping while their translation stays put.
        let angle = from.rotation.angle_between(to.rotation);
        if angle > 1e-4 {
            return angle / 90f32.to_radians();
        }
        0.0
    }
}

fn apply_vob_triggers(mut triggers: MessageReader<VobTrigger>, mut movers: Query<&mut GameMover>) {
    let _perf = perf_debug!();
    let targets: Vec<String> = triggers.read().map(|event| event.target.clone()).collect();
    if targets.is_empty() {
        return;
    }
    for mut mover in &mut movers {
        if targets
            .iter()
            .any(|target| mover.name.eq_ignore_ascii_case(target))
        {
            mover.trigger();
            info!(mover = %mover.name, target = mover.target, "mover triggered");
        }
    }
}

/// `zCTriggerWorldStart` events are delivered once the world is in game, when
/// the movers they start have been spawned.
fn fire_world_start_triggers(
    mut pending: ResMut<PendingWorldStartTriggers>,
    mut triggers: MessageWriter<VobTrigger>,
) {
    let _perf = perf_debug!();
    for target in pending.0.drain(..) {
        info!(target = %target, "world start trigger");
        triggers.write(VobTrigger { target });
    }
}

fn advance_movers(time: Res<Time>, mut movers: Query<(&mut GameMover, &mut Transform)>) {
    let _perf = perf_debug!();
    for (mut mover, mut transform) in &mut movers {
        if mover.current == mover.target {
            continue;
        }
        let from = mover.keyframes[mover.current];
        let to = mover.keyframes[mover.target];
        let duration = mover.segment_duration(&from, &to);
        if duration <= 0.0 {
            mover.finish_segment(&mut transform);
            continue;
        }
        mover.progress = (mover.progress + time.delta_secs() / duration).min(1.0);
        if mover.progress >= 1.0 {
            mover.finish_segment(&mut transform);
            if mover.looping {
                debug!(mover = %mover.name, keyframe = mover.current, "mover looped");
            } else {
                info!(mover = %mover.name, keyframe = mover.current, "mover reached keyframe");
            }
            continue;
        }
        let pose =
            Transform::from_translation(from.translation.lerp(to.translation, mover.progress))
                .with_rotation(from.rotation.slerp(to.rotation, mover.progress));
        *transform = mover.spawn.mul_transform(pose);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mover(behavior: MoverBehavior) -> GameMover {
        let keyframes = [0.0, 1.0, 2.0]
            .into_iter()
            .map(|x| Transform::from_xyz(x, 0.0, 0.0))
            .collect();
        GameMover::new(
            "test".to_string(),
            keyframes,
            Transform::IDENTITY,
            10.0,
            true,
            behavior,
        )
    }

    #[test]
    fn loop_trigger_starts_at_the_first_segment() {
        let mut mover = mover(MoverBehavior::LOOP);
        mover.trigger();
        assert!(mover.looping);
        assert_eq!((mover.current, mover.target), (0, 1));
    }

    #[test]
    fn loop_wraps_from_the_last_keyframe_to_the_first() {
        let mut mover = mover(MoverBehavior::LOOP);
        mover.trigger();
        let mut transform = Transform::IDENTITY;
        let mut visited = Vec::new();
        for _ in 0..4 {
            mover.finish_segment(&mut transform);
            visited.push(mover.current);
        }
        assert_eq!(visited, vec![1, 2, 0, 1]);
        assert_eq!((mover.current, mover.target), (1, 2));
    }

    #[test]
    fn a_second_loop_trigger_does_not_restart_the_cycle() {
        let mut mover = mover(MoverBehavior::LOOP);
        mover.trigger();
        mover.current = 2;
        mover.target = 0;
        mover.trigger();
        assert_eq!((mover.current, mover.target), (2, 0));
    }

    #[test]
    fn toggle_mover_keeps_the_toggle_reaction() {
        let mut mover = mover(MoverBehavior::TOGGLE);
        mover.trigger();
        assert_eq!((mover.current, mover.target), (0, 2));
        assert!(!mover.looping);
    }

    #[test]
    fn disabled_mover_ignores_triggers() {
        let mut mover = mover(MoverBehavior::LOOP);
        mover.enabled = false;
        mover.trigger();
        assert!(!mover.looping);
        assert_eq!((mover.current, mover.target), (0, 0));
    }
}
