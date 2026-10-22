mod camera;
mod flow;
mod sounds;
mod state;
mod ui;
pub use sounds::SpeechSounds;

use crate::input::InputMode;
use crate::zengin::script::bridge::ScriptChangesApplied;
use bevy::prelude::*;
use state::DialogRequest;
pub(crate) use state::DialogUiState;

pub struct GameDialogPlugin;

impl Plugin for GameDialogPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DialogUiState>();
        app.add_message::<DialogRequest>();
        app.add_systems(Startup, ui::override_default_font);
        app.add_systems(Startup, ui::spawn_dialog_ui);
        // The UI systems emit requests, `handle_dialog_requests` applies them
        // and runs the script before the script bridge drains the produced
        // outputs, `handle_dialog_outputs` consumes them in the same frame,
        // `aim_dialog_camera` points the camera at the current speaker and
        // `sync_dialog_ui` renders the result.
        app.add_systems(
            Update,
            (
                flow::show_closest_npc_dialog,
                state::handle_dialog_input,
                state::advance_dialog_playback,
                flow::handle_dialog_requests,
            )
                .chain()
                .before(ScriptChangesApplied),
        );
        app.add_systems(
            Update,
            (
                flow::handle_dialog_outputs,
                camera::aim_dialog_camera,
                ui::sync_dialog_ui,
                sync_input_mode,
            )
                .chain()
                .after(ScriptChangesApplied),
        );
    }
}

/// While a dialog or the inventory is open the input mode is GUI navigation,
/// otherwise gameplay.
pub(crate) fn sync_input_mode(
    dialog_ui: Res<DialogUiState>,
    inventory_ui: Res<crate::gui::InventoryUiState>,
    mut input_mode: ResMut<InputMode>,
) {
    let mode = if dialog_ui.is_open() || inventory_ui.is_open() {
        InputMode::Gui
    } else {
        InputMode::Gameplay
    };
    if *input_mode != mode {
        *input_mode = mode;
    }
}
