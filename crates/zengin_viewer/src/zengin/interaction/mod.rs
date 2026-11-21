//! Player interactions with the selected world object.

pub mod ranged;
pub mod selection;

mod actions;
mod combat;
mod debug;
mod loot;
mod player_mob;
mod use_mob;

use bevy::prelude::*;

pub(crate) use combat::{DamageTargetData, deal_damage};
pub(crate) use combat::{MeleeHit, NpcDied};
pub(crate) use debug::{grant_test_items, seed_test_journal};

use crate::game_state::AppState;
use crate::zengin::script::TransferItems;

pub struct PlayerInteractionPlugin;

impl Plugin for PlayerInteractionPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<TransferItems>();
        app.add_message::<NpcDied>();
        app.add_message::<MeleeHit>();
        app.add_systems(OnEnter(AppState::InGame), debug::seed_test_journal_on_start);
        app.add_systems(
            Update,
            (
                combat::melee_swing_hit,
                combat::npc_melee_swing_hit,
                crate::graphics::npc_hit_reaction_end,
                combat::handle_npc_death,
                actions::handle_transfer_items,
                actions::pickup_selected_item,
                loot::open_selected_chest,
                player_mob::start_auto_end_release
                    .before(player_mob::stand_up_player_from_mob)
                    .before(player_mob::advance_player_mob_release),
                player_mob::stand_up_player_from_mob,
                player_mob::close_looted_chest.before(player_mob::advance_player_mob_release),
                player_mob::advance_player_mob_release,
                player_mob::sync_player_mob_physics
                    .before(crate::gameplay::character::move_and_slide),
                use_mob::use_selected_mob,
                actions::toggle_player_weapons,
                debug::debug_equipment_on_start,
            )
                .run_if(in_state(AppState::InGame)),
        );
    }
}
