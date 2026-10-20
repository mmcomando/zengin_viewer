use bevy::prelude::*;

use super::data::AnimationData;

/// Playback control of a skeleton.
///
/// When the current animation ends, [`AnimationController::next`] is played;
/// if it is `None`, the current animation loops. [`AnimationController::interrupt_current`]
/// allows the queued `next` to replace a still playing animation immediately —
/// otherwise one-shot animations have to play to their end first.
///
/// What to play next is decided by other systems (the auto anim sequencer or
/// scripted sequences). The only way to change the played animation is [`AnimationController::queue_next`]; the playback switch
/// itself is performed by the animation update system.
#[derive(Component, Debug)]
pub struct AnimationController {
    /// Currently played animation
    current: Handle<AnimationData>,
    /// Animation to play when the current one ends; `None` loops the current one
    next: Option<Handle<AnimationData>>,
    /// Whether the queued `next` may replace `current` immediately
    interrupt_current: bool,
    /// Whether the root movement of the current animation moves the entity
    /// itself (Gothic-style root motion: the animation drives the position,
    /// the character controller only handles gravity and collisions).
    should_move_entity: bool,
}

impl AnimationController {
    pub fn new(current: Handle<AnimationData>) -> Self {
        Self {
            current,
            next: None,
            interrupt_current: false,
            should_move_entity: false,
        }
    }

    /// Queues the animation to play next. When the current animation ends it
    /// replaces `current`; with `interrupt_current` it replaces it immediately.
    /// Queuing the animation that is already playing is a no-op.
    pub fn queue_next(&mut self, handle: Handle<AnimationData>, interrupt_current: bool) {
        if self.current == handle {
            // debug!(
            //     "Animation({:?}) already playing, not queueing",
            //     self.current.path(),
            // );
            return;
        }
        // info!(
        //     "Animation change queued: {:?} -> {:?} (interrupt_current: {})",
        //     self.current.path(),
        //     handle.path(),
        //     interrupt_current,
        // );
        self.next = Some(handle);
        self.interrupt_current = interrupt_current;
    }

    /// Drops the queued `next` animation, the current one loops on.
    pub fn clear_next(&mut self) {
        self.next = None;
        self.interrupt_current = false;
    }

    pub fn current(&self) -> &Handle<AnimationData> {
        &self.current
    }

    pub fn next(&self) -> Option<&Handle<AnimationData>> {
        self.next.as_ref()
    }

    pub fn interrupt_current(&self) -> bool {
        self.interrupt_current
    }

    /// Takes the queued `next` animation out, leaving the controller to loop
    /// the current animation.
    pub fn take_next(&mut self) -> Option<Handle<AnimationData>> {
        self.next.take()
    }

    /// Switches the currently played animation to `handle`. Only for the
    /// playback system that applies a switch prepared with
    /// [`AnimationController::queue_next`] — all other systems must request
    /// animation changes through `queue_next` only.
    pub(crate) fn switch_to(&mut self, handle: Handle<AnimationData>) {
        info!("Animation switch_to({:?})", handle.path());
        self.current = handle;
        self.next = None;
        self.interrupt_current = false;
    }

    /// Enables or disables Gothic-style root motion for this skeleton.
    pub fn set_root_motion(&mut self, should_move_entity: bool) {
        self.should_move_entity = should_move_entity;
    }

    pub fn root_motion(&self) -> bool {
        self.should_move_entity
    }
}
