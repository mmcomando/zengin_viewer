//! Opening chests: the selected chest requests the loot UI on `Interact`.

use bevy::log::{info, warn};
use bevy::prelude::*;
use leafwing_input_manager::prelude::ActionState;

use super::selection::{Selection, SelectionKind};
use crate::gameplay::objects::PlayerCharacter;
use crate::gui::{GameLog, InventoryUiState, OpenLoot};
use crate::input::{GameplayAction, GameplayInputMarker};
use crate::zengin::npc_routines::NpcWeaponMode;
use crate::zengin::script::state::WeaponMode;
use crate::zengin::{DialogUiState, GameChest, GameNpc};

pub(super) fn open_selected_chest(
    action_state: Single<&ActionState<GameplayAction>, With<GameplayInputMarker>>,
    selection: Res<Selection>,
    player_npc: Query<&GameNpc, With<PlayerCharacter>>,
    player_weapon: Query<&NpcWeaponMode, With<PlayerCharacter>>,
    dialog_ui: Res<DialogUiState>,
    inventory_ui: Res<InventoryUiState>,
    chests: Query<&GameChest>,
    mut game_log: ResMut<GameLog>,
    mut requests: MessageWriter<OpenLoot>,
) {
    if !action_state.just_pressed(&GameplayAction::Interact) {
        return;
    }
    if dialog_ui.is_open() || inventory_ui.is_open() {
        return;
    }
    let weapon_mode = player_weapon
        .single()
        .map_or(WeaponMode::Fists, |mode| mode.0);
    if weapon_mode != WeaponMode::Fists {
        return;
    }
    if selection.kind != Some(SelectionKind::Chest) {
        return;
    }
    let Some(target) = selection.target else {
        return;
    };
    let Ok(chest) = chests.get(target) else {
        return;
    };
    let Ok(player_npc) = player_npc.single() else {
        return;
    };

    if chest.locked {
        warn!(
            "Chest({target}) vob({}) is locked, lock ignored",
            chest.name
        );
        game_log.add("Chest is locked (lock ignored)".to_string());
    }

    info!(
        "Player({}) opened chest({target}) vob({})",
        player_npc.instance_id(),
        chest.name
    );
    requests.write(OpenLoot {
        player: player_npc.instance_id(),
        chest: target,
    });
}
