//! Npc actions. Each action is a component placed on the npc by the queue;
//! removing [`ExecutingActionMarker`] cancels the action.

pub mod action;
pub mod attack;
pub mod enqueue;
pub mod flush;
pub mod goto;
pub mod goto_npc;
pub mod item_state;
pub mod queue;
pub mod stand_up;
pub mod start_state;
pub mod use_mob;
pub mod wait;

pub use action::{
    ExecutingActionMarker, drop_item_pose_for_action, remove_actions_on_marker_removed,
    warn_direct_action_removal,
};
pub use attack::{
    AttackAction, advance_npc_attack_swings, end_attack_action, run_attack_action,
    start_attack_action,
};
pub use enqueue::{
    clear_npc_actions, enqueue_align_actions, enqueue_attack_actions, enqueue_goto_actions,
    enqueue_goto_fp_actions, enqueue_goto_npc_actions, enqueue_npc_goto, enqueue_stand_up_actions,
    enqueue_start_state_actions, enqueue_use_item_state_actions, enqueue_use_mob_actions,
    enqueue_wait_actions,
};
pub use flush::flush_npc_action_queue;
pub use goto::{GoToWaypointAction, end_goto_action, run_goto_action, start_goto_action};
pub use goto_npc::{
    GoToNpcAction, end_goto_npc_action, run_goto_npc_action, start_goto_npc_action,
};
pub use item_state::{
    NpcItemUse, UseItemStateAction, advance_item_use_animations, run_use_item_state_action,
    sync_npc_item_body_state,
};
pub use queue::NpcActionQueue;
pub use stand_up::{StandUpAction, start_stand_up_action};
pub use start_state::{StartStateAction, run_start_state_action, start_start_state_action};
pub use use_mob::{
    MobUsePose, UseMobAction, advance_mob_use_pose, end_use_mob_action, enter_animation_name,
    mob_animation_name, mob_scheme_has_player_pose, release_mob_use, release_mob_use_actions,
    run_use_mob_action, start_use_mob_action,
};
pub use wait::{WaitAction, run_wait_action};
