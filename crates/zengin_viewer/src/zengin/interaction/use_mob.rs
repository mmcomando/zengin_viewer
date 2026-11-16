//! Using the selected interactive mob: `Interact` flips the mob state, which
//! plays its own animation and runs its `onStateFunc` hook.

use bevy::log::{info, warn};
use bevy::prelude::*;
use leafwing_input_manager::prelude::ActionState;

use super::selection::{Selection, SelectionKind};
use crate::gameplay::objects::PlayerCharacter;
use crate::gui::InventoryUiState;
use crate::input::{GameplayAction, GameplayInputMarker};
use crate::zengin::npc_routines::NpcWeaponMode;
use crate::zengin::script::component::ScriptResource;
use crate::zengin::script::state::WeaponMode;
use crate::zengin::{DialogUiState, GameMob, GameNpc, VobTrigger};

/// Toggles the selected door/lever/wheel between its two states. The npc that
/// would use it in the engine stays standing here, only the mob reacts.
pub(super) fn use_selected_mob(
    action_state: Single<&ActionState<GameplayAction>, With<GameplayInputMarker>>,
    selection: Res<Selection>,
    player_npc: Query<&GameNpc, With<PlayerCharacter>>,
    player_weapon: Query<&NpcWeaponMode, With<PlayerCharacter>>,
    dialog_ui: Res<DialogUiState>,
    inventory_ui: Res<InventoryUiState>,
    mut mobs: Query<&mut GameMob>,
    gothic_script: Res<ScriptResource>,
    mut vob_trigger: MessageWriter<VobTrigger>,
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
    if selection.kind != Some(SelectionKind::Mob) {
        return;
    }
    let Some(target) = selection.target else {
        return;
    };
    let Ok(player_npc) = player_npc.single() else {
        return;
    };
    let Ok(mut mob) = mobs.get_mut(target) else {
        return;
    };
    if mob.state_num <= 0 {
        warn!(
            "Mob({target}) vob({}) has no interaction states",
            mob.vob_name
        );
        return;
    }
    let state = i32::from(mob.state <= 0);
    let player = player_npc.instance_id();
    if !gothic_script
        .lock()
        .mob_condition_allows(player, &mob.condition_func)
    {
        info!(
            "Player({player}) mob({target}) use denied by condition_func({})",
            mob.condition_func
        );
        return;
    }
    mob.state = state;
    info!(
        "Player({player}) used mob({target}) vob({}) focus({}) condition_func({}) on_state_func({}) -> state({state})",
        mob.vob_name, mob.focus_name, mob.condition_func, mob.on_state_func,
    );
    if !mob.trigger_target.is_empty() && (mob.rewind || !mob.trigger_fired) {
        mob.trigger_fired = true;
        vob_trigger.write(VobTrigger {
            target: mob.trigger_target.clone(),
        });
    }
    gothic_script
        .lock()
        .call_mob_state_hook(player, &mob.on_state_func, state);
}
