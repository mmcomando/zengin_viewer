//! Player interactions with the selected world object (currently: killing npcs).

use bevy::prelude::*;

use crate::gameplay::distance::ClassifyDistance;
use crate::gameplay::objects::PlayerCharacter;
use crate::gui::game_log::{GameLog, GameLogEntry};
use crate::input::actions::{DebugAction, DebugInputMarker, GameplayAction, GameplayInputMarker};
use crate::zengin::npc_routines::NpcEquippedWeapon;
use crate::zengin::script::TransferItems;
use crate::zengin::script::component::ScriptResource;
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
                give_test_items,
                open_journal,
            ),
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
/// inventory (logged bottom-left), the equipped weapon is dropped on the ground
/// as a pickable item, and the npc entity is despawned.
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
    npcs: Query<(&GameNpc, &GlobalTransform, Option<&NpcEquippedWeapon>), Without<PlayerCharacter>>,
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
    let Ok((npc, global_tr, equipped)) = npcs.get(target) else {
        return;
    };
    let npc_id = npc.npc_instance_id;
    let position = global_tr.translation();
    let equipped = equipped.map(|equipped| equipped.0);

    let Ok(player_npc) = player_npc.single() else {
        return;
    };
    let player_id = player_npc.npc_instance_id;

    // Move the inventory to the player, it is logged by `handle_transfer_items`.
    transfer_items.write(TransferItems {
        instance_from_id: npc_id,
        instance_to_id: player_id,
    });

    // Drop the equipped weapon on the ground as a pickable item.
    if let Some(item) = equipped {
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
    let mut candidates: Vec<InstanceId> = script
        .get_loaded_state()
        .items_instances
        .iter()
        .filter(|(_, instance)| !instance.name.is_empty())
        .map(|(id, _)| *id)
        .collect();
    candidates.sort_unstable_by_key(|id| id.value());
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
