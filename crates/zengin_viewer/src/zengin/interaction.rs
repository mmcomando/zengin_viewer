//! Player interactions with the selected world object (currently: killing npcs).

use bevy::prelude::*;

use crate::gameplay::distance::ClassifyDistance;
use crate::gameplay::objects::PlayerCharacter;
use crate::gui::game_log::{GameLog, GameLogEntry};
use crate::input::actions::{DebugAction, DebugInputMarker, GameplayAction, GameplayInputMarker};
use crate::zengin::npc_routines::{NpcEquippedWeapon, NpcWeaponMode};
use crate::zengin::script::TransferItems;
use crate::zengin::script::component::ScriptResource;
use crate::zengin::script::state::WeaponMode;
use crate::zengin::script::symbol_id::InstanceId;
use crate::zengin::selection::{Selection, SelectionKind};
use crate::zengin::world::item_model_path;
use crate::zengin::zengin_resources::{MaterialHandles, ZenGinModelComponent};
use crate::zengin::{GameItem, GameNpc, ZenginVfs};
use leafwing_input_manager::prelude::ActionState;

pub struct PlayerInteractionPlugin;

impl Plugin for PlayerInteractionPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<TransferItems>();
        app.add_systems(
            Update,
            (
                kill_selected_npc,
                handle_transfer_items,
                pickup_selected_item,
                toggle_player_weapons,
                give_test_items,
                open_journal,
            ),
        );
    }
}

/// Toggles the player's equipped weapons between sheathed and drawn on
/// `EquipMelee`/`EquipRanged` (1/2): pressing draws the respective weapon into
/// the hand, pressing again sheathes it back to the hip/back. A warn is logged
/// when nothing of that kind is equipped.
fn toggle_player_weapons(
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
        let instance = match script.npc_instances().get(&player_id) {
            Some(instance) => instance,
            None => {
                warn!(
                    "No script instance for player({}) on weapon toggle",
                    player_id.value()
                );
                continue;
            }
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
fn handle_transfer_items(
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
            let name = script
                .get_name_by_id(item.value())
                .unwrap_or_else(|| item.value().to_string());
            game_log.write(GameLogEntry {
                text: format!("{name} x{count}"),
            });
        }
    }
}

/// Kills the selected npc on `K`: its inventory moves into the player's
/// inventory (logged bottom-left), the drawn weapon is dropped on the ground
/// as a pickable item (removed from the inventory first so it is not
/// duplicated), and the npc entity is despawned.
#[allow(clippy::too_many_arguments)]
fn kill_selected_npc(
    action_state: Single<&ActionState<DebugAction>, With<DebugInputMarker>>,
    mut selection: ResMut<Selection>,
    mut transfer_items: MessageWriter<TransferItems>,
    gothic_scripts: Res<ScriptResource>,
    vfs: Res<ZenginVfs>,
    mut handles: ResMut<MaterialHandles>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
    npcs: Query<
        (
            &GameNpc,
            &GlobalTransform,
            Option<&NpcEquippedWeapon>,
            Option<&NpcWeaponMode>,
        ),
        Without<PlayerCharacter>,
    >,
    player_npc: Query<&GameNpc, With<PlayerCharacter>>,
) {
    if !action_state.just_pressed(&DebugAction::KillSelected) {
        return;
    }
    if selection.kind != Some(SelectionKind::Npc) {
        return;
    }
    let Some(target) = selection.target else {
        return;
    };
    let Ok((npc, global_tr, equipped, weapon_mode)) = npcs.get(target) else {
        return;
    };
    let npc_id = npc.npc_instance_id;
    let position = global_tr.translation();
    // Only the drawn weapon drops on the ground; sheathed weapons stay part of
    // the inventory that moves to the player
    let dropped = match weapon_mode.map(|mode| mode.0).unwrap_or(WeaponMode::Fists) {
        WeaponMode::Melee => equipped.and_then(|equipped| equipped.melee),
        WeaponMode::Ranged => equipped.and_then(|equipped| equipped.ranged),
        WeaponMode::Fists => None,
    };

    let Ok(player_npc) = player_npc.single() else {
        return;
    };
    let player_id = player_npc.npc_instance_id;

    // Move the inventory to the player, it is logged by `handle_transfer_items`.
    transfer_items.write(TransferItems {
        instance_from_id: npc_id,
        instance_to_id: player_id,
    });

    // Drop the drawn weapon on the ground as a pickable item. Remove it from
    // the npc inventory first when present, otherwise the transfer below
    // would duplicate it.
    if let Some(item) = dropped {
        let mut script = gothic_scripts.lock();
        let removed = script.remove_inventory(npc_id, item, 1);
        if removed == 0 {
            warn!(
                "Dropped weapon item({}) of npc({}) was not in its inventory",
                item.value(),
                npc_id.value()
            );
        }
        drop(script);
        match item_model_path(&gothic_scripts, &vfs, item) {
            Some(path) => {
                let model_handle = handles.get_model_handle(&asset_server, &path, None);
                commands.spawn((
                    ClassifyDistance::default(),
                    ZenGinModelComponent {
                        model_handle,
                        ..default()
                    },
                    Visibility::default(),
                    Transform::from_translation(position),
                    GameItem {
                        item_instance_id: item,
                    },
                ));
            }
            None => warn!("No mesh path for dropped weapon item({item})"),
        }
    }

    info!("Killed npc({npc_id}), moved its inventory to player({player_id})");
    commands.entity(target).despawn();
    selection.target = None;
    selection.kind = None;
    selection.name.clear();
}

/// Picks up the selected item on `E`: it is added to the player's inventory
/// (logged bottom-left) and its world entity is despawned.
fn pickup_selected_item(
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
    let name = script
        .get_name_by_id(item_id.value())
        .unwrap_or_else(|| item_id.value().to_string());
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
fn open_journal(
    action_state: Single<&ActionState<GameplayAction>, With<GameplayInputMarker>>,
    mut game_log: ResMut<GameLog>,
) {
    if !action_state.just_pressed(&GameplayAction::Journal) {
        return;
    }
    game_log.add("Journal clicked".to_string());
    info!("Journal action triggered");
}

/// Grants a few items to the player on the debug key, for testing the
/// inventory UI without having to pick things up first.
fn give_test_items(
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

    const TEST_ITEM_COUNT: usize = 12;
    let mut script = gothic_script.lock();
    let mut weapons: Vec<InstanceId> = Vec::new();
    let mut others: Vec<InstanceId> = Vec::new();
    for (id, instance) in script.get_loaded_state().items_instances.iter() {
        if instance.name.is_empty() {
            continue;
        }
        let lowered = instance.name.to_lowercase();
        if lowered.starts_with("itmw_") || lowered.starts_with("itrw_") {
            weapons.push(*id);
        } else {
            others.push(*id);
        }
    }
    // Weapons first, so a known slot holds an equippable item
    weapons.sort_unstable_by_key(|id| id.value());
    others.sort_unstable_by_key(|id| id.value());
    let mut candidates = weapons;
    candidates.extend(others);
    if candidates.is_empty() {
        warn!("No item instances loaded, cannot grant test items");
        return;
    }

    let player_id = player_npc.npc_instance_id;
    let granted = candidates.iter().take(TEST_ITEM_COUNT).enumerate();
    let mut lines: Vec<String> = Vec::new();
    for (offset, &item) in granted {
        let count = 1 + (offset as i32 % 3);
        script.add_inventory(player_id, item, count);
        let name = script
            .get_name_by_id(item.value())
            .map(|symbol_name| crate::gui::inventory::display_item_name(&symbol_name))
            .unwrap_or_else(|| item.value().to_string());
        lines.push(format!("{name} x{count}"));
    }
    game_log.add(lines.join("\n"));
    info!(
        "Granted {} test item stack(s) to player({})",
        lines.len(),
        player_id
    );
}
