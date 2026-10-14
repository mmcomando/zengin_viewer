//! Player interactions with the selected world object (currently: killing npcs).

use bevy::prelude::*;

use crate::gameplay::distance::ClassifyDistance;
use crate::gameplay::objects::PlayerCharacter;
use crate::zengin::npc_routines::WeaponMeshPaths;
use crate::zengin::pickup_log::PickupLog;
use crate::zengin::script::component::{ScriptNpcState, ScriptResource};
use crate::zengin::selection::{Selection, SelectionKind};
use crate::zengin::zengin_resources::{MaterialHandles, ZenGinModelComponent};
use crate::zengin::{GameItem, GameNpc};

pub struct PlayerInteractionPlugin;

impl Plugin for PlayerInteractionPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (kill_selected_npc, pickup_selected_item));
    }
}

/// Kills the selected npc on `K`: its inventory moves into the player's
/// inventory (logged bottom-left), the equipped weapon is dropped on the ground
/// as a pickable item, and the npc entity is despawned.
///
/// The shared npc state and the script are locked separately, never nested, see
/// `script_host_ecs_bridge.md`.
#[allow(clippy::too_many_arguments)]
fn kill_selected_npc(
    keys: Res<ButtonInput<KeyCode>>,
    mut selection: ResMut<Selection>,
    mut pickup_log: ResMut<PickupLog>,
    npc_state: Res<ScriptNpcState>,
    gothic_script: Res<ScriptResource>,
    weapon_mesh_paths: Res<WeaponMeshPaths>,
    mut handles: ResMut<MaterialHandles>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
    npcs: Query<(&GameNpc, &GlobalTransform), Without<PlayerCharacter>>,
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
    let Ok((npc, global_tr)) = npcs.get(target) else {
        return;
    };
    let npc_id = npc.npc_instance_id;
    let position = global_tr.translation();

    // Read the equipped weapon from the shared state (its own lock).
    let equipped = {
        let Ok(live) = npc_state.0.lock() else {
            warn!("Shared npc state lock is poisoned, cannot kill npc({npc_id})");
            return;
        };
        live.equipped_weapon.get(&npc_id).copied()
    };

    let Ok(player_npc) = player_npc.single() else {
        return;
    };
    let player_id = player_npc.npc_instance_id;

    // Move the inventory to the player and resolve the dropped weapon's name.
    let (pickups, weapon) = {
        let mut script = gothic_script.lock();
        let moved = script.transfer_inventory(npc_id, player_id);
        let pickups: Vec<String> = moved
            .iter()
            .map(|(item, count)| {
                let name = script
                    .get_name_by_id(item.value())
                    .unwrap_or_else(|| item.value().to_string());
                format!("{name} x{count}")
            })
            .collect();
        let weapon = equipped.map(|item| {
            let name = script
                .get_name_by_id(item.value())
                .unwrap_or_else(|| item.value().to_string());
            (item, name)
        });
        (pickups, weapon)
    };
    for line in pickups {
        pickup_log.add(line);
    }

    // Drop the equipped weapon on the ground as a pickable item.
    if let Some((item, name)) = weapon {
        match weapon_mesh_paths.0.get(&item.value()) {
            Some(path) => {
                let model_handle = handles.get_model_handle(&asset_server, path, None);
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
                        name,
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
    mut pickup_log: ResMut<PickupLog>,
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
    let name = item.name.clone();
    gothic_script
        .lock()
        .add_inventory(player_npc.npc_instance_id, item_id, 1);
    pickup_log.add(format!("{name} x1"));

    info!(
        "Picked up item({item_id}) by player({})",
        player_npc.npc_instance_id
    );
    commands.entity(target).despawn();
    selection.target = None;
    selection.kind = None;
    selection.name.clear();
}
