//! FIFO of npc actions. The front action is executed every frame until it
//! reports completion; an empty queue leaves the npc standing still.

mod action;
mod attack;
mod enqueue;
mod goto;
mod item_state;
mod queue;
mod run;
mod stand_up;
mod use_mob;

pub(crate) use action::{remove_actions_on_marker_removed, warn_direct_action_removal};
pub(crate) use attack::{AttackAction, advance_npc_attack_swings};
pub(crate) use enqueue::{
    clear_npc_actions, enqueue_align_actions, enqueue_attack_actions, enqueue_goto_actions,
    enqueue_goto_fp_actions, enqueue_stand_up_actions, enqueue_use_item_state_actions,
    enqueue_use_mob_actions,
};
pub(crate) use goto::GoToWaypointAction;
pub(crate) use item_state::{
    NpcItemUse, UseItemStateAction, advance_item_use_animations, sync_npc_item_body_state,
};
pub(crate) use queue::{NpcAction, NpcActionQueue};
pub(crate) use run::run_npc_actions;
pub(crate) use stand_up::StandUpAction;
pub(crate) use use_mob::UseMobAction;
