//! Player interactions with the selected world object (currently: killing npcs).

use bevy::prelude::*;

use crate::gameplay::distance::ClassifyDistance;
use crate::gameplay::objects::PlayerCharacter;
use crate::gui::game_log::{GameLog, GameLogEntry};
use crate::zengin::npc_routines::NpcEquippedWeapon;
use crate::zengin::script::TransferItems;
use crate::zengin::script::component::ScriptResource;
use crate::zengin::selection::{Selection, SelectionKind};
use crate::zengin::world::item_model_path;
use crate::zengin::zengin_resources::{MaterialHandles, ZenGinModelComponent};
use crate::zengin::{GameItem, GameNpc, ZenginVfs};

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
                print_player_items,
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
    keys: Res<ButtonInput<KeyCode>>,
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
    if !keys.just_pressed(KeyCode::KeyK) {
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
    keys: Res<ButtonInput<KeyCode>>,
    mut selection: ResMut<Selection>,
    mut game_log: ResMut<GameLog>,
    gothic_script: Res<ScriptResource>,
    mut commands: Commands,
    items: Query<&GameItem>,
    player_npc: Query<&GameNpc, With<PlayerCharacter>>,
) {
    if !keys.just_pressed(KeyCode::KeyE) {
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

/// Prints the whole player inventory into the item log on `P`.
fn print_player_items(
    keys: Res<ButtonInput<KeyCode>>,
    mut game_log: ResMut<GameLog>,
    gothic_script: Res<ScriptResource>,
    player_npc: Query<&GameNpc, With<PlayerCharacter>>,
) {
    if !keys.just_pressed(KeyCode::KeyP) {
        return;
    }
    let Ok(player_npc) = player_npc.single() else {
        return;
    };

    let mut script = gothic_script.lock();
    let mut items = script.inventory(player_npc.npc_instance_id);
    if items.is_empty() {
        game_log.add(format!(
            "player({}) inventory is empty",
            player_npc.npc_instance_id
        ));
        return;
    }

    items.sort_by_key(|(item, _count)| *item);
    let lines: Vec<String> = items
        .iter()
        .map(|(item, count)| {
            let name = script
                .get_name_by_id(item.value())
                .unwrap_or_else(|| item.value().to_string());
            format!("{name} x{count}")
        })
        .collect();
    game_log.add(lines.join("\n"));
}
