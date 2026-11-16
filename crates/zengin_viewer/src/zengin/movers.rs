//! World movers (`zCMover`): visual vobs that travel along their keyframes when
//! a trigger event names them. A button (`oCMobInter.triggerTarget`) opens the
//! gate by emitting [`VobTrigger`] addressed to the mover's vob name.

use bevy::log::info;
use bevy::prelude::*;

pub struct MoversPlugin;

impl Plugin for MoversPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<VobTrigger>();
        app.add_systems(Update, (apply_vob_triggers, advance_movers));
    }
}

/// Trigger event addressed to every vob whose name matches `target`.
#[derive(Message, Debug, Clone)]
pub struct VobTrigger {
    pub target: String,
}

/// `zCMover` moving between its keyframes.
#[derive(Component, Debug, Clone)]
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
    /// `zCTrigger.startEnabled`.
    pub enabled: bool,
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
    ) -> Self {
        let base = keyframes.first().copied().unwrap_or(spawn);
        let inverse_base = base.compute_affine().inverse();
        let keyframes = keyframes
            .iter()
            .map(|keyframe| {
                Transform::from_matrix(Mat4::from(inverse_base * keyframe.compute_affine()))
            })
            .collect();
        Self {
            name,
            spawn,
            keyframes,
            current: 0,
            target: 0,
            progress: 0.0,
            speed,
            enabled,
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
    let targets: Vec<String> = triggers.read().map(|event| event.target.clone()).collect();
    if targets.is_empty() {
        return;
    }
    for mut mover in &mut movers {
        if targets
            .iter()
            .any(|target| mover.name.eq_ignore_ascii_case(target))
        {
            mover.toggle();
            info!(mover = %mover.name, target = mover.target, "mover triggered");
        }
    }
}

fn advance_movers(time: Res<Time>, mut movers: Query<(&mut GameMover, &mut Transform)>) {
    for (mut mover, mut transform) in &mut movers {
        if mover.current == mover.target {
            continue;
        }
        let from = mover.keyframes[mover.current];
        let to = mover.keyframes[mover.target];
        let duration = mover.segment_duration(&from, &to);
        if duration <= 0.0 {
            mover.current = mover.target;
            *transform = mover.spawn.mul_transform(to);
            continue;
        }
        mover.progress = (mover.progress + time.delta_secs() / duration).min(1.0);
        if mover.progress >= 1.0 {
            mover.current = mover.target;
            mover.progress = 0.0;
            *transform = mover.spawn.mul_transform(to);
            info!(mover = %mover.name, keyframe = mover.current, "mover reached keyframe");
            continue;
        }
        let pose =
            Transform::from_translation(from.translation.lerp(to.translation, mover.progress))
                .with_rotation(from.rotation.slerp(to.rotation, mover.progress));
        *transform = mover.spawn.mul_transform(pose);
    }
}
