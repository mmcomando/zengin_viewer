//! Player interactions with the selected world object.

pub mod ranged;
pub mod selection;

mod actions;
mod combat;
mod debug;
mod loot;

use bevy::prelude::*;

pub(crate) use combat::{MeleeHit, NpcDied};
pub(crate) use combat::{NpcDamageData, damage_npc};
pub(crate) use debug::{grant_test_items, seed_test_journal};

use crate::game_state::AppState;
use crate::zengin::script::TransferItems;
use crate::zengin::script::component::GameScriptLoaded;

pub struct PlayerInteractionPlugin;

impl Plugin for PlayerInteractionPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<TransferItems>();
        app.add_message::<NpcDied>();
        app.add_message::<MeleeHit>();
        app.add_systems(
            OnEnter(AppState::InGame),
            debug::seed_test_journal_on_start.after(GameScriptLoaded),
        );
        app.add_systems(
            Update,
            (
                combat::melee_swing_hit,
                crate::graphics::npc_hit_reaction_end,
                combat::handle_npc_death,
                actions::handle_transfer_items,
                actions::pickup_selected_item,
                loot::open_selected_chest,
                actions::toggle_player_weapons,
                debug::debug_equipment_on_start,
            )
                .run_if(in_state(AppState::InGame)),
        );
    }
}
