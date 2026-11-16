//! FIFO of npc actions. The front action is executed every frame until it
//! reports completion; an empty queue leaves the npc standing still.

mod attack;
mod enqueue;
mod goto;
mod queue;
mod run;

pub(crate) use attack::advance_npc_attack_swings;
pub(crate) use enqueue::{
    clear_npc_actions, enqueue_align_actions, enqueue_attack_actions, enqueue_goto_actions,
    enqueue_goto_fp_actions, enqueue_stand_up_actions, enqueue_use_mob_actions,
};
pub(crate) use queue::{NpcAction, NpcActionQueue};
pub(crate) use run::run_npc_actions;
