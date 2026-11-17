//! Npc actions. Each action is a component placed on the npc by the queue;
//! removing [`ExecutingActionMarker`] cancels the action.

mod action;
mod attack;
mod enqueue;
mod flush;
mod goto;
mod item_state;
mod queue;
mod stand_up;
mod use_mob;

pub(crate) use action::{
    drop_item_pose_for_action, remove_actions_on_marker_removed, warn_direct_action_removal,
};
pub(crate) use attack::{
    AttackAction, advance_npc_attack_swings, end_attack_action, run_attack_action,
    start_attack_action,
};
pub(crate) use enqueue::{
    clear_npc_actions, enqueue_align_actions, enqueue_attack_actions, enqueue_goto_actions,
    enqueue_goto_fp_actions, enqueue_stand_up_actions, enqueue_use_item_state_actions,
    enqueue_use_mob_actions,
};
pub(crate) use flush::flush_npc_action_queue;
pub(crate) use goto::{GoToWaypointAction, end_goto_action, run_goto_action, start_goto_action};
pub(crate) use item_state::{
    NpcItemUse, UseItemStateAction, advance_item_use_animations, run_use_item_state_action,
    sync_npc_item_body_state,
};
pub(crate) use queue::NpcActionQueue;
pub(crate) use stand_up::{StandUpAction, start_stand_up_action};
pub(crate) use use_mob::{
    UseMobAction, end_use_mob_action, run_use_mob_action, start_use_mob_action,
};
