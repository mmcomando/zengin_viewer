use std::collections::VecDeque;

use bevy::prelude::*;
use daedalus_wasm::daedalus::FightAiMove;
use rand_mt::Mt;

use crate::graphics::MeleeAttackKind;
use crate::zengin::script::string_value::StringCanon;
use crate::zengin::script::symbol_id::InstanceId;

/// The action at the front is executed every frame until it is done.
#[derive(Component, Default)]
pub(crate) struct NpcActionQueue {
    actions: VecDeque<NpcAction>,
    /// Front action has already been started (its `NpcPath` was inserted).
    started: bool,
    attack: NpcAttackTrace,
    /// Attack randomness, seeded per npc on first use so replays stay
    /// deterministic while clones of one instance do not attack in lockstep.
    rng: Mt,
    rng_seeded: bool,
}

impl NpcActionQueue {
    /// Appends an action, `false` when the queue is full. A `GoToWaypoint`
    /// replaces the `GoToWaypoint` at the back instead of stacking on it.
    pub(crate) fn push(&mut self, action: NpcAction) -> bool {
        match action {
            NpcAction::GoToWaypoint { waypoint } => self.push_goto(waypoint),
            NpcAction::Attack { target } => self.push_attack(target),
        }
    }

    fn push_goto(&mut self, waypoint: StringCanon) -> bool {
        if let Some(NpcAction::GoToWaypoint { waypoint: last }) = self.actions.back_mut() {
            *last = waypoint;
            // The replaced action was the active front, its path must be replanned.
            if self.actions.len() == 1 {
                self.started = false;
            }
            return true;
        }
        if self.actions.len() >= MAX_QUEUED_ACTIONS {
            return false;
        }
        self.actions.push_back(NpcAction::GoToWaypoint { waypoint });
        true
    }

    /// An attack preempts everything else: it clears the queue and starts right
    /// away. A repeated request for the attack already at the front is a no-op
    /// so an ongoing swing is not restarted.
    fn push_attack(&mut self, target: Entity) -> bool {
        if let Some(NpcAction::Attack { target: current }) = self.actions.front()
            && *current == target
        {
            return true;
        }
        self.actions.clear();
        self.started = false;
        self.attack.swings.clear();
        self.actions.push_back(NpcAction::Attack { target });
        true
    }

    pub(super) fn front(&self) -> Option<&NpcAction> {
        self.actions.front()
    }

    pub(super) fn pop_front(&mut self) {
        self.actions.pop_front();
        self.started = false;
        self.attack.swings.clear();
    }

    /// Drops every queued action (`Npc_ClearAIQueue`).
    pub(super) fn clear(&mut self) {
        self.actions.clear();
        self.started = false;
        self.attack.swings.clear();
    }

    pub(super) fn started(&self) -> bool {
        self.started
    }

    pub(super) fn mark_started(&mut self) {
        self.started = true;
    }

    pub(super) fn attack(&self) -> &NpcAttackTrace {
        &self.attack
    }

    pub(super) fn attack_mut(&mut self) -> &mut NpcAttackTrace {
        &mut self.attack
    }

    /// Per-npc attack generator, seeded from the instance and entity.
    pub(super) fn attack_rng(&mut self, instance: InstanceId, entity: Entity) -> &mut Mt {
        if !self.rng_seeded {
            self.rng_seeded = true;
            let seed = entity.to_bits() as u32 ^ instance.value() as u32;
            self.rng = Mt::new(seed);
        }
        &mut self.rng
    }
}

/// Swings of the active attack action, the front one is playing.
#[derive(Debug, Default)]
pub(super) struct NpcAttackTrace {
    pub(super) swings: VecDeque<MeleeAttackKind>,
    /// FAI move the swings were expanded from; a repeat pick is avoided.
    pub(super) last_move: Option<FightAiMove>,
}

#[derive(Clone, Debug)]
pub(crate) enum NpcAction {
    GoToWaypoint {
        waypoint: StringCanon,
    },
    /// Chase and melee the target entity until it dies, the target is gone or
    /// it is farther away than the attack abandonment distance.
    Attack {
        target: Entity,
    },
}

/// Upper bound of queued actions, including the action at the front.
const MAX_QUEUED_ACTIONS: usize = 8;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_replaces_the_last_goto() {
        let mut queue = NpcActionQueue::default();
        assert!(queue.push(waypoint("wp0")));
        assert!(queue.push(waypoint("wp1")));

        assert_eq!(queue.actions.len(), 1);
        let Some(NpcAction::GoToWaypoint { waypoint }) = queue.front() else {
            panic!("front action missing");
        };
        assert_eq!(waypoint.as_str(), "wp1");
    }

    #[test]
    fn replacing_the_active_goto_restarts_it() {
        let mut queue = NpcActionQueue::default();
        queue.push(waypoint("wp0"));
        queue.mark_started();

        queue.push(waypoint("wp1"));

        assert!(!queue.started());
        let Some(NpcAction::GoToWaypoint { waypoint }) = queue.front() else {
            panic!("front action missing");
        };
        assert_eq!(waypoint.as_str(), "wp1");
    }

    #[test]
    fn attack_preempts_queued_actions() {
        let mut queue = NpcActionQueue::default();
        queue.push(waypoint("wp0"));

        assert!(queue.push(NpcAction::Attack {
            target: Entity::PLACEHOLDER,
        }));

        assert_eq!(queue.actions.len(), 1);
        assert!(matches!(queue.front(), Some(NpcAction::Attack { .. })));
        assert!(!queue.started());
    }

    #[test]
    fn repeated_attack_does_not_restart() {
        let mut queue = NpcActionQueue::default();
        let target = Entity::PLACEHOLDER;
        queue.push(NpcAction::Attack { target });
        queue.mark_started();

        assert!(queue.push(NpcAction::Attack { target }));

        assert_eq!(queue.actions.len(), 1);
        assert!(queue.started());
    }

    #[test]
    fn clear_drops_every_action_and_resets_started() {
        let mut queue = NpcActionQueue::default();
        queue.push(waypoint("wp0"));
        queue.push(NpcAction::Attack {
            target: Entity::PLACEHOLDER,
        });
        queue.mark_started();

        queue.clear();

        assert!(queue.front().is_none());
        assert!(!queue.started());
    }

    #[test]
    fn attack_swings_are_dropped_with_the_action() {
        let mut queue = NpcActionQueue::default();
        queue.push(NpcAction::Attack {
            target: Entity::PLACEHOLDER,
        });
        queue.attack_mut().swings.push_back(MeleeAttackKind::Left);

        queue.pop_front();

        assert!(queue.attack().swings.is_empty());

        queue.push(NpcAction::Attack {
            target: Entity::PLACEHOLDER,
        });
        queue.attack_mut().swings.push_back(MeleeAttackKind::Right);

        queue.clear();

        assert!(queue.attack().swings.is_empty());
    }

    fn waypoint(name: &str) -> NpcAction {
        NpcAction::GoToWaypoint {
            waypoint: StringCanon::new(name),
        }
    }
}
