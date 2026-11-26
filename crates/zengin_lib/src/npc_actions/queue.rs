use std::collections::VecDeque;

use bevy::prelude::*;

use crate::npc_actions::attack::AttackAction;
use crate::npc_actions::goto::GoToWaypointAction;
use crate::npc_actions::goto_npc::GoToNpcAction;
use crate::npc_actions::item_state::UseItemStateAction;
use crate::npc_actions::stand_up::StandUpAction;
use crate::npc_actions::start_state::StartStateAction;
use crate::npc_actions::use_mob::UseMobAction;
use crate::npc_actions::wait::WaitAction;
use crate::script::string_value::StringCanon;
use crate::script::symbol_id::{FunctionId, InstanceId};

/// Queued actions, the front one is moved onto the npc when it is free.
#[derive(Component, Default)]
pub struct NpcActionQueue {
    actions: VecDeque<QueuedAction>,
}

impl NpcActionQueue {
    /// A new destination replaces the queued one instead of stacking behind it.
    pub fn push_goto(&mut self, waypoint: StringCanon) -> bool {
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

    /// Repeated `AI_GotoNpc` requests for the same target do not stack.
    pub fn push_goto_npc(&mut self, target: Entity) -> bool {
        let queued = self.actions.iter().any(
            |action| matches!(action, QueuedAction::GoToNpc(queued) if queued.target == target),
        );
        if queued {
            return true;
        }
        self.push_back(QueuedAction::GoToNpc(GoToNpcAction { target }))
    }

    /// Only the latest request for a mob scheme matters; the routine loop
    /// re-issues it until the body state changes.
    pub fn push_use_mob(&mut self, scheme: StringCanon, state: i32) -> bool {
        let queued = self.actions.iter().position(
            |action| matches!(action, QueuedAction::UseMob(queued) if queued.scheme == scheme),
        );
        if let Some(index) = queued {
            if let Some(QueuedAction::UseMob(action)) = self.actions.get_mut(index) {
                action.state = state;
            }
            return true;
        }
        self.push_back(QueuedAction::UseMob(UseMobAction::npc(scheme, state)))
    }

    /// Only an identical `AI_UseItemToState` request is deduplicated.
    pub fn push_use_item_state(
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

    /// Waits always append; several waits in a row are legitimate.
    pub fn push_wait(&mut self, remaining: f32) -> bool {
        self.push_back(QueuedAction::Wait(WaitAction { remaining }))
    }

    /// A repeated `AI_StandUp` is a no-op.
    pub fn push_stand_up(&mut self) -> bool {
        if self
            .actions
            .iter()
            .any(|action| matches!(action, QueuedAction::StandUp(_)))
        {
            return true;
        }
        self.push_back(QueuedAction::StandUp(StandUpAction))
    }

    /// A repeated `AI_StartState` for the same state is a no-op.
    pub fn push_start_state(&mut self, state: FunctionId, other: Option<InstanceId>) -> bool {
        if self.actions.iter().any(
            |action| matches!(action, QueuedAction::StartState(queued) if queued.state == state),
        ) {
            return true;
        }
        self.push_back(QueuedAction::StartState(StartStateAction { state, other }))
    }

    /// Attacks always append; the `AiState` loop gate prevents duplicates.
    pub fn push_attack(&mut self, target: Entity) -> bool {
        self.push_back(QueuedAction::Attack(Box::new(AttackAction::new(target))))
    }

    /// Whether an attack on `target` is already queued.
    pub fn has_attack(&self, target: Entity) -> bool {
        self.actions
            .iter()
            .any(|action| matches!(action, QueuedAction::Attack(queued) if queued.target == target))
    }

    fn push_back(&mut self, action: QueuedAction) -> bool {
        if self.actions.len() >= MAX_QUEUED_ACTIONS {
            return false;
        }
        self.actions.push_back(action);
        true
    }

    /// Whether any action is pending.
    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }

    pub fn pop_front(&mut self) -> Option<QueuedAction> {
        self.actions.pop_front()
    }

    /// Drops every queued action (`Npc_ClearAIQueue`).
    pub fn clear(&mut self) {
        self.actions.clear();
    }
}

pub enum QueuedAction {
    GoTo(GoToWaypointAction),
    GoToNpc(GoToNpcAction),
    Attack(Box<AttackAction>),
    UseMob(UseMobAction),
    UseItemState(UseItemStateAction),
    Wait(WaitAction),
    StandUp(StandUpAction),
    StartState(StartStateAction),
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
    fn goto_npc_does_not_stack_the_same_target() {
        let mut queue = NpcActionQueue::default();
        let target = Entity::PLACEHOLDER;
        assert!(queue.push_goto_npc(target));
        assert!(queue.push_goto_npc(target));

        assert_eq!(queue.actions.len(), 1);
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
    fn start_state_does_not_stack_the_same_state() {
        let mut queue = NpcActionQueue::default();
        let state = FunctionId::new(1).expect("valid state id");
        let other = InstanceId::new(2).expect("valid instance id");

        assert!(queue.push_start_state(state, Some(other)));
        assert!(queue.push_start_state(state, Some(other)));

        assert_eq!(queue.actions.len(), 1);
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
