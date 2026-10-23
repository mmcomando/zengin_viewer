//! Player interaction actions: weapon toggle, item transfer between
//! inventories, picking up the selected item and the journal trigger.

use bevy::log::{info, warn};
use bevy::prelude::*;
use leafwing_input_manager::prelude::ActionState;

use super::selection::{Selection, SelectionKind};
use crate::gameplay::objects::PlayerCharacter;
use crate::gui::{GameLog, GameLogEntry};
use crate::input::{GameplayAction, GameplayInputMarker};
use crate::zengin::script::TransferItems;
use crate::zengin::script::component::ScriptResource;
use crate::zengin::script::state::WeaponMode;
use crate::zengin::{GameItem, GameNpc};

/// Toggles the player's equipped weapons between sheathed and drawn on
/// `EquipMelee`/`EquipRanged`: pressing draws the respective weapon into
/// the hand, pressing again sheathes it back to the hip/back.
pub(super) fn toggle_player_weapons(
    action_state: Single<&ActionState<GameplayAction>, With<GameplayInputMarker>>,
    player_npc: Query<&GameNpc, With<PlayerCharacter>>,
    gothic_script: Res<ScriptResource>,
    mut game_log: ResMut<GameLog>,
) {
    let Ok(player_npc) = player_npc.single() else {
        return;
    };
    let player_id = player_npc.instance_id();

    for (action, ranged) in [
        (GameplayAction::EquipMelee, false),
        (GameplayAction::EquipRanged, true),
    ] {
        if !action_state.just_pressed(&action) {
            continue;
        }
        let mut script = gothic_script.lock();
        let Some(instance) = script.npc_instances().get(&player_id) else {
            warn!(
                "No script instance for player({}) on weapon toggle",
                player_id.value()
            );
            continue;
        };
        let equipped = if ranged {
            instance.equipped_ranged.is_some()
        } else {
            instance.equipped_melee.is_some()
        };
        let drawn = match instance.weapon_mode {
            Some(WeaponMode::Melee) => !ranged,
            Some(WeaponMode::Ranged) => ranged,
            _ => false,
        };
        let name = if ranged { "ranged" } else { "melee" };
        if !equipped {
            game_log.add(format!("No {name} weapon equipped"));
            info!(
                "Weapon toggle for player({}) ignored: no {name} weapon equipped",
                player_id.value()
            );
            continue;
        }
        if drawn {
            script.sheathe_weapon(player_id);
        } else {
            script.draw_weapon(player_id, ranged);
        }
        info!(
            "Player({}) {} its {} weapon",
            player_id.value(),
            if drawn { "sheathed" } else { "drew" },
            name
        );
    }
}

/// Moves all items between the requested instance inventories and logs each
/// moved stack bottom-left.
pub(super) fn handle_transfer_items(
    mut requests: MessageReader<TransferItems>,
    mut game_log: MessageWriter<GameLogEntry>,
    gothic_script: Res<ScriptResource>,
) {
    for request in requests.read() {
        let mut script = gothic_script.lock();
        let moved = script.transfer_inventory(request.instance_from_id, request.instance_to_id);
        if moved.is_empty() {
            continue;
        }
        info!(
            "Moved {} item stack(s) from instance({}) to instance({})",
            moved.len(),
            request.instance_from_id,
            request.instance_to_id
        );
        for (item, count) in moved {
            let name = script.item_log_name(item);
            game_log.write(GameLogEntry {
                text: format!("{name} x{count}"),
            });
        }
    }
}

/// Picks up the selected item on `E`: it is added to the player's inventory
/// (logged bottom-left) and its world entity is despawned.
pub(super) fn pickup_selected_item(
    action_state: Single<&ActionState<GameplayAction>, With<GameplayInputMarker>>,
    mut selection: ResMut<Selection>,
    mut game_log: ResMut<GameLog>,
    gothic_script: Res<ScriptResource>,
    mut commands: Commands,
    items: Query<&GameItem>,
    player_npc: Query<&GameNpc, With<PlayerCharacter>>,
) {
    if !action_state.just_pressed(&GameplayAction::Interact) {
        return;
    }
    if selection.kind != Some(SelectionKind::Item) {
        return;
    }
    let Some(target) = selection.target else {
        return;
    };
    let Ok(item) = items.get(target) else {
        return;
    };
    let Ok(player_npc) = player_npc.single() else {
        return;
    };

    let item_id = item.item_instance_id;
    let mut script = gothic_script.lock();
    let name = script.item_log_name(item_id);
    script.add_inventory(player_npc.npc_instance_id, item_id, 1);
    drop(script);
    game_log.add(format!("{name} x1"));

    info!(
        "Picked up item({item_id}) by player({})",
        player_npc.npc_instance_id
    );
    commands.entity(target).despawn();
    selection.target = None;
    selection.kind = None;
    selection.name.clear();
}

/// Placeholder journal trigger: logs that the journal was requested until a
/// real journal UI exists.
pub(super) fn open_journal(
    action_state: Single<&ActionState<GameplayAction>, With<GameplayInputMarker>>,
    mut game_log: ResMut<GameLog>,
) {
    if !action_state.just_pressed(&GameplayAction::Journal) {
        return;
    }
    game_log.add("Journal clicked".to_string());
    info!("Journal action triggered");
}
