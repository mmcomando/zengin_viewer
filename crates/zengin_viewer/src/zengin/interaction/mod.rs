//! Player interactions with the selected world object.

pub mod ranged;
pub mod selection;

mod actions;
mod combat;
mod debug;
mod lockpick;
mod loot;
mod player_mob;
mod use_mob;

use bevy::prelude::*;

pub(crate) use combat::{DamageTargetData, deal_damage};
pub(crate) use combat::{MeleeHit, NpcDied};
pub(crate) use debug::{grant_test_items, seed_test_journal};

use crate::input::world_input_enabled;
use crate::zengin::AppState;
use crate::zengin::script::bridge::ScriptChangesApplied;

pub struct PlayerInteractionPlugin;

impl Plugin for PlayerInteractionPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<NpcDied>();
        app.add_message::<MeleeHit>();
        app.add_message::<combat::NpcDeathResolved>();
        app.add_systems(OnEnter(AppState::InGame), debug::seed_test_journal_on_start);
        app.add_systems(
            Update,
            (
                combat::npc_melee_swing_hit,
                crate::zengin::npc_hit_reaction_end,
                combat::drop_npc_death_weapon,
                player_mob::start_auto_end_release
                    .before(player_mob::stand_up_player_from_mob)
                    .before(player_mob::advance_player_mob_release),
                player_mob::close_looted_chest.before(player_mob::advance_player_mob_release),
                lockpick::open_lockpicked_chest,
                player_mob::release_stale_player_mob_use,
                player_mob::advance_player_mob_release,
                player_mob::sync_player_mob_physics
                    .before(crate::gameplay::character::move_and_slide),
                debug::debug_equipment_on_start,
            )
                .run_if(in_state(AppState::InGame)),
        );
        // Script driven deaths are queued by the script host; forwarding them
        // after the change systems keeps the script lock contention out of the
        // death handler itself.
        app.add_systems(
            Update,
            combat::script_npc_deaths
                .after(ScriptChangesApplied)
                .run_if(in_state(AppState::InGame)),
        );
        // Death handling takes the game script lock several times. Run it in
        // `PostUpdate`, which cannot overlap `Update`, so it does not contend
        // with the world-wide mob systems that also lock the script every frame.
        app.add_systems(
            PostUpdate,
            combat::handle_npc_death.run_if(in_state(AppState::InGame)),
        );
        // Player-driven interactions read gameplay input, so they stay inert
        // while a view owns the input. Ordering them after script changes makes
        // the gate see a dialog opened earlier in the same frame.
        app.add_systems(
            Update,
            (
                combat::melee_swing_hit,
                actions::pickup_selected_item,
                loot::open_selected_chest,
                loot::open_selected_corpse,
                use_mob::use_selected_mob,
                actions::toggle_player_weapons,
                player_mob::stand_up_player_from_mob.before(player_mob::advance_player_mob_release),
            )
                .run_if(in_state(AppState::InGame))
                .run_if(world_input_enabled)
                .after(ScriptChangesApplied),
        );
    }
}
