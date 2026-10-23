//! Player interactions with the selected world object.

pub mod ranged;
pub mod selection;

mod actions;
mod combat;
mod debug;

use bevy::prelude::*;

pub(crate) use combat::{MeleeHit, NpcDied};
pub(crate) use combat::{NpcDamageData, damage_npc};

use crate::zengin::script::TransferItems;

pub struct PlayerInteractionPlugin;

impl Plugin for PlayerInteractionPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<TransferItems>();
        app.add_message::<NpcDied>();
        app.add_message::<MeleeHit>();
        app.add_systems(
            Update,
            (
                combat::melee_swing_hit,
                crate::graphics::npc_hit_reaction_end,
                combat::handle_npc_death,
                actions::handle_transfer_items,
                actions::pickup_selected_item,
                actions::toggle_player_weapons,
                debug::give_test_items,
                debug::debug_equipment_on_start,
                actions::open_journal,
            ),
        );
    }
}
