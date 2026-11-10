//! NPC fighter tactic tables (`FIGHT.DAT`) shared by npc combat.

use std::path::PathBuf;

use bevy::prelude::*;
use daedalus_wasm::daedalus::FightAiTables;

use crate::zengin::common::gothic2_dir;

/// Fighter tactic tables from the compiled scripts folder, empty when the file
/// could not be read.
#[derive(Resource, Debug, Default)]
pub(crate) struct FightAiResource(pub(crate) FightAiTables);

/// Loads the tables beside the main script. A missing or unreadable file is
/// logged and yields empty tables, npc combat then keeps its fallback.
pub(crate) fn load_fight_ai_tables() -> FightAiResource {
    let path = PathBuf::from(gothic2_dir() + "/_work/Data/Scripts/_compiled/FIGHT.DAT");
    match FightAiTables::load(&path) {
        Ok(tables) => FightAiResource(tables),
        Err(err) => {
            warn!(
                "Fight AI tables({}) not loaded, npc attack variety is disabled: {err}",
                path.display()
            );
            FightAiResource::default()
        }
    }
}
