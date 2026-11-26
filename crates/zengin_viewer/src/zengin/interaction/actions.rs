//! Player interaction actions: weapon toggle, item transfer between
//! inventories and picking up the selected item.

use bevy::log::{info, warn};
use bevy::prelude::*;
use leafwing_input_manager::prelude::ActionState;

use super::selection::{Selection, SelectionKind};
use crate::gui::GameLog;
use crate::input::{GameplayAction, GameplayInputMarker};
use crate::zengin::PlayerCharacter;
use crate::zengin::script::component::ScriptResource;
use crate::zengin::script::state::WeaponMode;
use crate::zengin::script::symbol_id::InstanceId;
use crate::zengin::{GameItem, GameNpc};
use dev_util::perf_debug;

/// Toggles the player's equipped weapons between sheathed and drawn on
/// `EquipMelee`/`EquipRanged`, and toggles the last used weapon on
/// `DrawWeapon`. Pressing draws the weapon into the hand, pressing again
/// sheathes it back to the hip/back.
pub(super) fn toggle_player_weapons(
    action_state: Single<&ActionState<GameplayAction>, With<GameplayInputMarker>>,
    player_npc: Query<&GameNpc, With<PlayerCharacter>>,
    gothic_script: Res<ScriptResource>,
    mut game_log: ResMut<GameLog>,
) {
    let _perf = perf_debug!();
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
        toggle_weapon(&gothic_script, &mut game_log, player_id, action, ranged);
    }
    if action_state.just_pressed(&GameplayAction::DrawWeapon) {
        toggle_last_weapon(&gothic_script, player_id);
    }
}

/// Draws the last used weapon when it is sheathed, sheathes it when it is
/// already drawn. Falls back to the melee weapon or fists.
#[allow(clippy::cognitive_complexity)]
fn toggle_last_weapon(gothic_script: &ScriptResource, player_id: InstanceId) {
    let mut script = gothic_script.lock();
    let Some(target) = script
        .npc_state(player_id)
        .map(|npc| npc.equipment.last_used_weapon())
    else {
        warn!(
            "No script instance for player({}) on draw weapon",
            player_id.value()
        );
        return;
    };
    let drawn = script
        .npc_state(player_id)
        .and_then(|npc| npc.equipment.weapon_mode)
        == Some(target);
    if target == WeaponMode::Fists {
        info!(
            "Player({}) has no weapon to draw, fighting with fists",
            player_id.value()
        );
        script.sheathe_weapon(player_id);
        return;
    }
    if drawn {
        script.sheathe_weapon(player_id);
        return;
    }
    let ranged = target == WeaponMode::Ranged;
    script.draw_weapon(player_id, ranged);
}

/// Applies one weapon toggle press for the player.
fn toggle_weapon(
    gothic_script: &ScriptResource,
    game_log: &mut GameLog,
    player_id: InstanceId,
    action: GameplayAction,
    ranged: bool,
) {
    let Some((equipped, drawn, name)) = weapon_state(gothic_script, player_id, ranged) else {
        warn!(
            "No script instance for player({}) on weapon toggle({action:?})",
            player_id.value()
        );
        return;
    };
    let mut script = gothic_script.lock();
    if !equipped {
        game_log.add(format!("No {name} weapon equipped"));
        info!(
            "Weapon toggle for player({}) ignored: no {name} weapon equipped",
            player_id.value()
        );
        return;
    }
    if drawn {
        script.sheathe_weapon(player_id);
    } else {
        script.draw_weapon(player_id, ranged);
    }
}

/// Equipped/drawn state of the player's `{ranged,melee}` weapon. Returns
/// `None` when the player has no script instance.
fn weapon_state(
    gothic_script: &ScriptResource,
    player_id: InstanceId,
    ranged: bool,
) -> Option<(bool, bool, &'static str)> {
    let script = gothic_script.lock();
    let instance = script.npc_state(player_id)?;
    let equipped = if ranged {
        instance.equipment.ranged.is_some()
    } else {
        instance.equipment.melee.is_some()
    };
    let drawn = match instance.equipment.weapon_mode {
        Some(WeaponMode::Melee) => !ranged,
        Some(WeaponMode::Ranged) => ranged,
        _ => false,
    };
    let name = if ranged { "ranged" } else { "melee" };
    Some((equipped, drawn, name))
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
    let _perf = perf_debug!();
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

    let item_id = item.instance_id();
    let mut script = gothic_script.lock();
    let name = script.item_log_name(item_id);
    script.add_inventory(player_npc.instance_id(), item_id, 1);
    drop(script);
    game_log.add(format!("{name} x1"));

    info!(
        "Picked up item({item_id}) by player({})",
        player_npc.instance_id()
    );
    commands.entity(target).despawn();
    selection.target = None;
    selection.kind = None;
    selection.name.clear();
}
