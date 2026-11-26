//! Debug helpers: hardcoded test items and the debug equipment granted at
//! game start, driven by debug input actions.

use bevy::log::{info, warn};
use bevy::prelude::*;

use crate::GameNpc;
use crate::PlayerCharacter;
use crate::gui::GameLog;
use crate::script::component::ScriptResource;
use crate::script::journal::{JournalSection, JournalStatus};
use crate::script::manager::{ScriptManager, TALENT_PICKLOCK};
use crate::script::symbol_id::InstanceId;
use dev_util::perf_debug;

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
    ("ITAR_XARDAS", 1),
    ("ITPO_HEALTH_01", 5),
    ("ITPO_MANA_01", 5),
    ("ITRI_STR_01", 1),
    ("ITAM_HP_01", 1),
    ("ITKE_LOCKPICK", 10),
    ("ITKE_KEY_01", 1),
    ("ItKe_Storage", 1),
    ("ITMISWORDRAW", 1),
];

/// Hardcoded journal topics seeded by [`seed_test_journal`], with the sections,
/// statuses and entries needed to exercise every journal category.
const TEST_JOURNAL: &[(&str, JournalSection, JournalStatus, &[&str])] = &[
    (
        "Return to the tower",
        JournalSection::Mission,
        JournalStatus::Running,
        &[
            "I must return to the tower to meet Xardas.",
            "Xardas surely knows what is happening on the island.",
        ],
    ),
    (
        "The Dragon Lord",
        JournalSection::Mission,
        JournalStatus::Success,
        &[
            "I have defeated the Dragon Lord.",
            "The road to the tower lies open.",
        ],
    ),
    (
        "Dragon Hunters",
        JournalSection::Mission,
        JournalStatus::Failed,
        &["I failed to convince the Dragon Hunters to help."],
    ),
    (
        "Loneliness and helplessness",
        JournalSection::Note,
        JournalStatus::Running,
        &["I feel lonely and helpless.", "I should look for allies."],
    ),
    (
        "Bennet is in prison",
        JournalSection::Note,
        JournalStatus::Running,
        &[
            "Bennet has been imprisoned in Khorinis.",
            "I must find a way to free him.",
        ],
    ),
];

/// Seeds [`TEST_JOURNAL`] into the script journal at startup so the journal UI
/// has content to show without progressing through the story.
pub fn seed_test_journal_on_start(gothic_script: Res<ScriptResource>) {
    let _perf = perf_debug!();
    let mut script = gothic_script.lock();
    let count = seed_test_journal(&mut script);
    info!("Seeded {count} test journal topic(s) at startup");
}

/// Seeds [`TEST_JOURNAL`] into the journal, returning the number of topics it
/// contains. Topics that already exist are left untouched, so seeding again
/// does not duplicate their entries.
pub fn seed_test_journal(script: &mut ScriptManager) -> usize {
    let journal = script.journal_mut();
    for (name, section, status, entries) in TEST_JOURNAL {
        if !journal.create_topic((*name).to_string(), *section) {
            continue;
        }
        journal.set_topic_status(name, *status);
        for entry in *entries {
            journal.add_entry(name, (*entry).to_string());
        }
    }
    TEST_JOURNAL.len()
}

/// Grants the hardcoded [`TEST_ITEMS`] to the player, for testing combat and the
/// inventory UI without having to pick things up first. Returns one log line per
/// granted stack.
pub fn grant_test_items(script: &mut ScriptManager, player_id: InstanceId) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for (item_name, count) in TEST_ITEMS {
        let Some(item) = grant_test_item(script, player_id, item_name, *count) else {
            warn!("Test item {item_name} not found among the loaded instances");
            continue;
        };
        lines.push(format!("{} x{count}", item_display_name(script, item)));
    }
    if lines.is_empty() {
        warn!("No test item granted to player({})", player_id);
    }
    lines
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
        .find(|(_, instance)| {
            instance
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .name
                .eq_ignore_ascii_case(item_name)
        })
        .map(|(id, _)| *id)?;
    script.add_inventory(player_id, item, count);
    Some(item)
}

/// Display name of an item for the game log.
fn item_display_name(script: &mut ScriptManager, item: InstanceId) -> String {
    script.item_log_name(item)
}

/// Grants the debug equipment to the player once its entity appears and equips
/// the first melee and ranged weapon from [`TEST_ITEMS`] (sheathed, the player
/// draws them with the weapon toggle inputs).
#[allow(clippy::cognitive_complexity)]
pub fn debug_equipment_on_start(
    gothic_script: Res<ScriptResource>,
    player_npc: Query<&GameNpc, (With<PlayerCharacter>, Added<GameNpc>)>,
    mut game_log: ResMut<GameLog>,
) {
    let _perf = perf_debug!();
    let Ok(player_npc) = player_npc.single() else {
        return;
    };
    let player_id = player_npc.instance_id();
    let mut script = gothic_script.lock();

    let mut lines: Vec<String> = Vec::new();
    let mut melee = None;
    let mut ranged = None;
    let mut armor = None;
    for (item_name, count) in TEST_ITEMS {
        let Some(item) = grant_test_item(&mut script, player_id, item_name, *count) else {
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
        } else if item_name.starts_with("ITAR_") {
            armor = armor.or(Some(item));
        }
        lines.push(format!("{} x{count}", item_display_name(&mut script, item)));
    }
    for (_, weapon) in [(false, melee), (true, ranged)] {
        let Some(weapon) = weapon else {
            continue;
        };
        script.equip_item(player_id, weapon);
    }
    script
        .npc_state_mut(player_id)
        .talents
        .set_skill(TALENT_PICKLOCK, 1);
    // if let Some(armor) = armor {
    //     info!(
    //         "Player({}) equipped debug armor({})",
    //         player_id.value(),
    //         armor.value()
    //     );
    //     script.equip_item(player_id, armor);
    // }
    drop(script);
    game_log.add(format!("Debug equipment:\n{}", lines.join("\n")));
    info!("Granted debug equipment to player({})", player_id);
}
