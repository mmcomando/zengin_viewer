use bevy::log::{info, warn};
use bevy::prelude::*;
use leafwing_input_manager::prelude::ActionState;

use crate::gui::{InventoryUiState, JournalUiState};
use crate::input::{GuiAction, GuiInputMarker, SharedAction, SharedInputMarker};
use crate::zengin::PlayerCharacter;
use crate::zengin::{DialogUiState, GameNpc, ScriptResource};

use super::data::StatsSnapshot;
use super::state::StatsUiState;
use dev_util::perf_debug;

/// The dialog, inventory and journal take precedence over the stats screen.
pub(super) fn toggle_stats(
    action_state: Single<&ActionState<SharedAction>, With<SharedInputMarker>>,
    dialog_ui: Res<DialogUiState>,
    inventory_ui: Res<InventoryUiState>,
    journal_ui: Res<JournalUiState>,
    mut state: ResMut<StatsUiState>,
    gothic_script: Res<ScriptResource>,
    player_npc: Query<&GameNpc, With<PlayerCharacter>>,
) {
    let _perf = perf_debug!();
    if !action_state.just_pressed(&SharedAction::Stats) {
        return;
    }
    if state.is_open() {
        state.close();
        return;
    }
    if dialog_ui.is_open() || inventory_ui.is_open() || journal_ui.is_open() {
        return;
    }

    let Ok(player_npc) = player_npc.single() else {
        warn!("Stats menu: the player npc is not spawned");
        return;
    };
    let snapshot = StatsSnapshot::from_npc(player_npc, &gothic_script);
    info!(
        "Stats menu opened: guild({}), level({})",
        snapshot.guild(),
        snapshot.level()
    );
    state.open(snapshot);
}

pub(super) fn close_stats_on_cancel(
    action_state: Single<&ActionState<GuiAction>, With<GuiInputMarker>>,
    mut state: ResMut<StatsUiState>,
) {
    let _perf = perf_debug!();
    if state.is_open() && action_state.just_pressed(&GuiAction::CancelAction) {
        state.close();
    }
}
