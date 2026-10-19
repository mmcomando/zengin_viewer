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
        app.add_message::<NpcDied>();
        app.add_systems(
            Update,
            (
                hit_selected_npc,
                handle_npc_death,
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

/// An npc died (hitpoints reached 0): its inventory moves to the player, the
/// drawn weapon drops on the ground and the npc entity is despawned.
#[derive(Message)]
pub struct NpcDied {
    pub entity: Entity,
    pub instance_id: InstanceId,
    /// Position where the dropped weapon is placed.
    pub position: Vec3,
}

/// Deals damage with the drawn weapon to the selected npc on `DealDamage` (F)
/// or on `Interact` while a melee weapon is drawn. With a ranged weapon drawn
/// `Interact` starts the bow attack sequence instead, handled by the ranged
/// combat module. The damage is the drawn weapon's `damagetotal`; without a
/// drawn weapon or a selected npc nothing happens. The npc dies when its
/// hitpoints reach 0.
#[allow(clippy::too_many_arguments)]
fn hit_selected_npc(
    action_state: Single<&ActionState<GameplayAction>, With<GameplayInputMarker>>,
    selection: Res<Selection>,
    npcs: Query<(&GameNpc, &GlobalTransform), Without<PlayerCharacter>>,
    player_npc: Query<&GameNpc, With<PlayerCharacter>>,
    player_weapon: Query<&NpcWeaponMode, With<PlayerCharacter>>,
    gothic_script: Res<ScriptResource>,
    mut game_log: ResMut<GameLog>,
    mut npc_died: MessageWriter<NpcDied>,
) {
    let weapon_mode = player_weapon
        .single()
        .map(|mode| mode.0)
        .unwrap_or(WeaponMode::Fists);
    // Ranged attacks go through the bow attack sequence, not instant damage.
    let melee_interact =
        weapon_mode == WeaponMode::Melee && action_state.just_pressed(&GameplayAction::Interact);
    if !action_state.just_pressed(&GameplayAction::DealDamage) && !melee_interact {
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
    let Ok(player_npc) = player_npc.single() else {
        return;
    };
    let player_id = player_npc.npc_instance_id;
    let npc_id = npc.npc_instance_id;

    let mut script = gothic_script.lock();
    let weapon = match weapon_mode {
        WeaponMode::Melee => script
            .npc_instances()
            .get(&player_id)
            .and_then(|player| player.equipped_melee),
        _ => {
            game_log.add("No weapon drawn");
            info!(
                "Deal damage ignored: player({}) has no weapon drawn",
                player_id.value()
            );
            return;
        }
    };
    let Some(weapon) = weapon else {
        warn!(
            "Deal damage ignored: player({}) weapon mode without equipped weapon",
            player_id.value()
        );
        return;
    };
    let Some(damage) = script.item_damage_total(weapon) else {
        warn!(
            "Deal damage ignored: no damage data for weapon item({})",
            weapon.value()
        );
        return;
    };
    let Some(hitpoints) = script.change_hitpoints(npc_id, -damage) else {
        warn!(
            "Deal damage ignored: no hitpoints data for npc({})",
            npc_id.value()
        );
        return;
    };
    info!(
        "Player({}) dealt {damage} damage to npc({}), hitpoints now {hitpoints}",
        player_id.value(),
        npc_id.value()
    );
    game_log.add(format!("Dealt {damage} damage"));
    if hitpoints <= 0 {
        npc_died.write(NpcDied {
            entity: target,
            instance_id: npc_id,
            position: global_tr.translation(),
        });
    }
}

/// Handles npc death: the inventory moves to the player, the drawn weapon
/// drops on the ground (removed from the inventory first so it is not
/// duplicated) and the npc entity is despawned.
#[allow(clippy::too_many_arguments)]
fn handle_npc_death(
    mut deaths: MessageReader<NpcDied>,
    mut selection: ResMut<Selection>,
    mut transfer_items: MessageWriter<TransferItems>,
    gothic_scripts: Res<ScriptResource>,
    vfs: Res<ZenginVfs>,
    mut handles: ResMut<MaterialHandles>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
    npcs: Query<
        (
            Option<&GameNpc>,
            Option<&NpcEquippedWeapon>,
            Option<&NpcWeaponMode>,
        ),
        Without<PlayerCharacter>,
    >,
    player_npc: Query<&GameNpc, With<PlayerCharacter>>,
) {
    for death in deaths.read() {
        let Ok((npc, equipped, weapon_mode)) = npcs.get(death.entity) else {
            continue;
        };
        let npc_id = npc
            .map(|npc| npc.npc_instance_id)
            .unwrap_or(death.instance_id);
        let position = death.position;
        // Only the drawn weapon drops on the ground; sheathed weapons stay
        // part of the inventory that moves to the player
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

        // Drop the drawn weapon on the ground as a pickable item. Remove it
        // from the npc inventory first when present, otherwise the transfer
        // below would duplicate it.
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

        info!("Npc({npc_id}) died, moved its inventory to player({player_id})");
        commands.entity(death.entity).despawn();
        if selection.target == Some(death.entity) {
            selection.target = None;
            selection.kind = None;
            selection.name.clear();
        }
    }
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

    let player_id = player_npc.npc_instance_id;
    let mut script = gothic_script.lock();
    let mut lines: Vec<String> = Vec::new();
    for (item_name, count) in TEST_ITEMS {
        // Item instances are keyed by id, resolve the hardcoded symbol name
        // against the loaded instances.
        let Some(item) = script
            .get_loaded_state()
            .items_instances
            .iter()
            .find(|(_, instance)| instance.name.eq_ignore_ascii_case(item_name))
            .map(|(id, _)| *id)
        else {
            warn!("Test item {item_name} not found among the loaded instances");
            continue;
        };
        script.add_inventory(player_id, item, *count);
        let name = script
            .get_name_by_id(item.value())
            .map(|symbol_name| crate::gui::inventory::display_item_name(&symbol_name))
            .unwrap_or_else(|| item.value().to_string());
        lines.push(format!("{name} x{count}"));
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
