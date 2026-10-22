//! Debug helpers: hardcoded test items and the debug equipment granted at
//! game start, driven by debug input actions.

use bevy::log::{info, warn};
use bevy::prelude::*;
use leafwing_input_manager::prelude::ActionState;

use crate::gameplay::objects::PlayerCharacter;
use crate::gui::GameLog;
use crate::input::{DebugAction, DebugInputMarker};
use crate::zengin::GameNpc;
use crate::zengin::script::component::ScriptResource;
use crate::zengin::script::manager::ScriptManager;
use crate::zengin::script::symbol_id::InstanceId;

/// Hardcoded test items granted by [`give_test_items`]: one weapon of each
/// kind, ammo, gold and a few extras worth testing with.
const TEST_ITEMS: &[(&str, i32)] = &[
    ("ITMW_1H_MIL_SWORD", 1),
    ("ITMW_2H_SLD_SWORD", 1),
    ("ITMW_1H_SLD_AXE", 1),
    ("ITMW_2H_SLD_AXE", 1),
    ("ITRW_BOW_H_01", 1),
    ("ITRW_MIL_CROSSBOW", 1),
    ("ITRW_ARROW", 25),
    ("ITRW_BOLT", 25),
    ("ITMI_GOLD", 50),
    ("ITAR_MIL_M", 1),
    ("ITPO_HEALTH_01", 5),
    ("ITPO_MANA_01", 5),
    ("ITRI_STR_01", 1),
    ("ITAM_HP_01", 1),
];

/// Grants the hardcoded [`TEST_ITEMS`] to the player on the debug key, for
/// testing combat and the inventory UI without having to pick things up first.
pub(super) fn give_test_items(
    action_state: Single<&ActionState<DebugAction>, With<DebugInputMarker>>,
    mut game_log: ResMut<GameLog>,
    gothic_script: Res<ScriptResource>,
    player_npc: Query<&GameNpc, With<PlayerCharacter>>,
) {
    if !action_state.just_pressed(&DebugAction::GiveTestItems) {
        return;
    }
    let Ok(player_npc) = player_npc.single() else {
        return;
    };

    let player_id = player_npc.instance_id();
    let mut script = gothic_script.lock();
    let mut lines: Vec<String> = Vec::new();
    for (item_name, count) in TEST_ITEMS {
        let Some(item) = grant_test_item(&mut *script, player_id, item_name, *count) else {
            warn!("Test item {item_name} not found among the loaded instances");
            continue;
        };
        lines.push(format!(
            "{} x{count}",
            item_display_name(&mut *script, item)
        ));
    }
    if lines.is_empty() {
        warn!("No test item granted to player({})", player_id);
        return;
    }
    game_log.add(lines.join("\n"));
    info!(
        "Granted {} test item stack(s) to player({})",
        lines.len(),
        player_id
    );
}

/// Resolves a hardcoded item symbol name against the loaded instances and
/// adds `count` of it to the player's inventory, returning the item id.
fn grant_test_item(
    script: &mut ScriptManager,
    player_id: InstanceId,
    item_name: &str,
    count: i32,
) -> Option<InstanceId> {
    let item = script
        .get_loaded_state()
        .items_instances
        .iter()
        .find(|(_, instance)| instance.name.eq_ignore_ascii_case(item_name))
        .map(|(id, _)| *id)?;
    script.add_inventory(player_id, item, count);
    Some(item)
}

/// Display name of an item for the game log.
fn item_display_name(script: &mut ScriptManager, item: InstanceId) -> String {
    script
        .get_name_by_id(item.value())
        .map(|symbol_name| crate::gui::display_item_name(&symbol_name))
        .unwrap_or_else(|| item.value().to_string())
}

/// Grants the debug equipment to the player once at the start of a game and
/// equips the first melee and ranged weapon from [`TEST_ITEMS`] (sheathed, the
/// player draws them with the weapon toggle inputs).
pub(super) fn debug_equipment_on_start(
    mut done: Local<bool>,
    gothic_script: Res<ScriptResource>,
    player_npc: Query<&GameNpc, With<PlayerCharacter>>,
    mut game_log: ResMut<GameLog>,
) {
    if *done {
        return;
    }
    let Ok(player_npc) = player_npc.single() else {
        return;
    };
    let player_id = player_npc.instance_id();
    let mut script = gothic_script.lock();
    // The player script instance is created when the game world spawns, that
    // marks the start of the game.
    if !script.npc_instances().contains_key(&player_id) {
        return;
    }

    let mut lines: Vec<String> = Vec::new();
    let mut melee = None;
    let mut ranged = None;
    for (item_name, count) in TEST_ITEMS {
        let Some(item) = grant_test_item(&mut *script, player_id, item_name, *count) else {
            warn!("Test item {item_name} not found among the loaded instances");
            continue;
        };
        // Skip the ranged ammo when picking the ranged weapon to equip.
        let is_ammo = matches!(
            item_name.to_ascii_lowercase().as_str(),
            "itrw_arrow" | "itrw_bolt"
        );
        if item_name.starts_with("ITMW_") {
            melee = melee.or(Some(item));
        } else if item_name.starts_with("ITRW_") && !is_ammo {
            ranged = ranged.or(Some(item));
        }
        lines.push(format!(
            "{} x{count}",
            item_display_name(&mut *script, item)
        ));
    }
    for (_, weapon) in [(false, melee), (true, ranged)] {
        let Some(weapon) = weapon else {
            continue;
        };
        script.equip_item(player_id, weapon);
    }
    drop(script);
    *done = true;
    game_log.add(format!("Debug equipment:\n{}", lines.join("\n")));
    info!("Granted debug equipment to player({})", player_id);
}
