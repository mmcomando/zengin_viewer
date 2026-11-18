use std::collections::VecDeque;

use bevy::prelude::*;

use crate::zengin::npc_actions::attack::AttackAction;
use crate::zengin::npc_actions::goto::GoToWaypointAction;
use crate::zengin::npc_actions::item_state::UseItemStateAction;
use crate::zengin::npc_actions::stand_up::StandUpAction;
use crate::zengin::npc_actions::use_mob::UseMobAction;
use crate::zengin::script::string_value::StringCanon;
use crate::zengin::script::symbol_id::InstanceId;

/// Queued actions, the front one is moved onto the npc when it is free.
#[derive(Component, Default)]
pub(crate) struct NpcActionQueue {
    actions: VecDeque<QueuedAction>,
}

impl NpcActionQueue {
    /// A new destination replaces the queued one instead of stacking behind it.
    pub(crate) fn push_goto(&mut self, waypoint: StringCanon) -> bool {
        let queued = self
            .actions
            .iter()
            .rposition(|action| matches!(action, QueuedAction::GoTo(_)));
        if let Some(index) = queued {
            if let Some(QueuedAction::GoTo(action)) = self.actions.get_mut(index) {
                action.waypoint = waypoint;
            }
            return true;
        }
        self.push_back(QueuedAction::GoTo(GoToWaypointAction { waypoint }))
    }

    /// Only the latest request for a mob scheme matters; the routine loop
    /// re-issues it until the body state changes.
    pub(crate) fn push_use_mob(&mut self, scheme: StringCanon, state: i32) -> bool {
        let queued = self.actions.iter().position(
            |action| matches!(action, QueuedAction::UseMob(queued) if queued.scheme == scheme),
        );
        if let Some(index) = queued {
            if let Some(QueuedAction::UseMob(action)) = self.actions.get_mut(index) {
                action.state = state;
            }
            return true;
        }
        self.push_back(QueuedAction::UseMob(UseMobAction { scheme, state }))
    }

    /// Only an identical `AI_UseItemToState` request is deduplicated.
    pub(crate) fn push_use_item_state(
        &mut self,
        item: InstanceId,
        scheme: StringCanon,
        state: i32,
    ) -> bool {
        let queued = self.actions.iter().any(|action| {
            matches!(action, QueuedAction::UseItemState(queued)
                if queued.item == item && queued.state == state)
        });
        if queued {
            return true;
        }
        self.push_back(QueuedAction::UseItemState(UseItemStateAction {
            item,
            scheme,
            state,
        }))
    }

    /// A repeated `AI_StandUp` is a no-op.
    pub(crate) fn push_stand_up(&mut self) -> bool {
        if self
            .actions
            .iter()
            .any(|action| matches!(action, QueuedAction::StandUp(_)))
        {
            return true;
        }
        self.push_back(QueuedAction::StandUp(StandUpAction))
    }

    /// Attacks always append; the `AiState` loop gate prevents duplicates.
    pub(crate) fn push_attack(&mut self, target: Entity) -> bool {
        self.push_back(QueuedAction::Attack(Box::new(AttackAction::new(target))))
    }

    fn push_back(&mut self, action: QueuedAction) -> bool {
        if self.actions.len() >= MAX_QUEUED_ACTIONS {
            return false;
        }
        self.actions.push_back(action);
        true
    }

    /// Whether any action is pending.
    pub(crate) fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }

    pub(super) fn pop_front(&mut self) -> Option<QueuedAction> {
        self.actions.pop_front()
    }

    /// Drops every queued action (`Npc_ClearAIQueue`).
    pub(crate) fn clear(&mut self) {
        self.actions.clear();
    }
}

pub(crate) enum QueuedAction {
    GoTo(GoToWaypointAction),
    Attack(Box<AttackAction>),
    UseMob(UseMobAction),
    UseItemState(UseItemStateAction),
    StandUp(StandUpAction),
}

/// Upper bound of queued actions, including the action at the front.
const MAX_QUEUED_ACTIONS: usize = 8;

#[cfg(test)]
mod tests {
    use super::*;

    fn waypoint(name: &str) -> StringCanon {
        StringCanon::new(name)
    }

    fn item(item: i32) -> InstanceId {
        InstanceId::new(item).expect("valid item instance id")
    }

    #[test]
    fn queue_replaces_the_last_goto() {
        let mut queue = NpcActionQueue::default();
        assert!(queue.push_goto(waypoint("wp0")));
        assert!(queue.push_goto(waypoint("wp1")));

        assert_eq!(queue.actions.len(), 1);
        let Some(QueuedAction::GoTo(action)) = queue.actions.front() else {
            panic!("front action missing");
        };
        assert_eq!(action.waypoint.as_str(), "wp1");
    }

    #[test]
    fn attack_always_appends() {
        let mut queue = NpcActionQueue::default();
        let target = Entity::PLACEHOLDER;
        assert!(queue.push_attack(target));
        assert!(queue.push_attack(target));

        assert_eq!(queue.actions.len(), 2);
    }

    #[test]
    fn use_mob_dedupes_the_last_request() {
        let mut queue = NpcActionQueue::default();
        assert!(queue.push_use_mob(waypoint("BENCH"), 1));
        assert!(queue.push_use_mob(waypoint("BENCH"), 1));

        assert_eq!(queue.actions.len(), 1);
    }

    #[test]
    fn use_mob_replaces_the_queued_state_of_its_scheme() {
        let mut queue = NpcActionQueue::default();
        queue.push_use_mob(waypoint("BENCH"), 1);
        queue.push_goto(waypoint("wp0"));

        assert!(queue.push_use_mob(waypoint("BENCH"), -1));

        assert_eq!(queue.actions.len(), 2);
        assert!(matches!(
            queue.actions.front(),
            Some(QueuedAction::UseMob(action)) if action.state == -1
        ));
    }

    #[test]
    fn use_item_state_dedupes_the_same_request() {
        let mut queue = NpcActionQueue::default();
        assert!(queue.push_use_item_state(item(1), waypoint("LUTE"), 1));
        assert!(queue.push_use_item_state(item(1), waypoint("LUTE"), 1));

        assert_eq!(queue.actions.len(), 1);
    }

    #[test]
    fn use_item_state_keeps_distinct_states() {
        let mut queue = NpcActionQueue::default();
        queue.push_use_item_state(item(1), waypoint("FIRESPIT"), 5);
        queue.push_use_item_state(item(1), waypoint("FIRESPIT"), 0);

        assert_eq!(queue.actions.len(), 2);
    }

    #[test]
    fn stand_up_does_not_stack() {
        let mut queue = NpcActionQueue::default();
        assert!(queue.push_stand_up());
        assert!(queue.push_stand_up());

        assert_eq!(queue.actions.len(), 1);
    }

    #[test]
    fn goto_replaces_a_queued_goto_behind_other_actions() {
        let mut queue = NpcActionQueue::default();
        queue.push_use_mob(waypoint("BENCH"), 1);
        queue.push_goto(waypoint("wp0"));

        assert!(queue.push_goto(waypoint("wp1")));

        assert_eq!(queue.actions.len(), 2);
        assert!(matches!(
            queue.actions.back(),
            Some(QueuedAction::GoTo(action)) if action.waypoint.as_str() == "wp1"
        ));
    }

    #[test]
    fn clear_drops_every_action() {
        let mut queue = NpcActionQueue::default();
        queue.push_goto(waypoint("wp0"));
        queue.push_attack(Entity::PLACEHOLDER);

        queue.clear();

        assert!(queue.actions.front().is_none());
    }

    #[test]
    fn queue_is_capped() {
        let mut queue = NpcActionQueue::default();
        for _ in 0..MAX_QUEUED_ACTIONS {
            assert!(queue.push_attack(Entity::PLACEHOLDER));
        }

        assert!(!queue.push_attack(Entity::PLACEHOLDER));
    }
}
