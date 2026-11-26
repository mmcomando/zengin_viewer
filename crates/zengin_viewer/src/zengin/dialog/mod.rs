mod camera;
mod flow;
mod sounds;
mod state;
mod ui;

pub(crate) use sounds::SpeechSounds;

pub(crate) use state::DialogRequest;
pub(crate) use state::DialogUiState;
pub(crate) use state::DialogVoice;

use crate::zengin::AppState;
use crate::zengin::gameplay::camera::EditorMode;
use crate::zengin::script::bridge::ScriptChangesApplied;
use crate::zengin::script::component::ScriptResource;
use crate::zengin::script::state::DialogLine;
use bevy::prelude::*;
use dev_util::external_messages::pump_external_messages;
use dev_util::perf_debug;
use input_core::InputMode;

pub struct GameDialogPlugin;

impl Plugin for GameDialogPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DialogUiState>();
        app.init_resource::<camera::DialogCameraShots>();
        app.add_message::<DialogRequest>();
        app.add_message::<DialogVoice>();
        app.add_systems(Startup, ui::override_default_font);
        app.add_systems(Startup, ui::spawn_dialog_ui);
        // The UI systems emit requests, `handle_dialog_requests` applies them
        // and runs the script; the dialog line pump below forwards the produced
        // `AI_Output` lines so `handle_dialog_outputs` consumes them in the
        // same frame. `aim_dialog_camera` picks a shot for the current speaker
        // and `sync_dialog_ui` renders the result.
        app.add_systems(
            Update,
            (
                flow::show_requested_npc_dialog,
                flow::show_closest_npc_dialog,
                state::handle_dialog_input,
                state::advance_dialog_playback,
                flow::handle_dialog_requests,
            )
                .chain()
                .before(ScriptChangesApplied)
                .run_if(in_state(AppState::InGame)),
        );
        // `handle_dialog_outputs` consumes the output capture flag once per
        // frame, so the lines an option produced have to be there in that same
        // frame.
        app.add_systems(
            Update,
            pump_external_messages::<DialogLine>
                .in_set(ScriptChangesApplied)
                .run_if(in_state(AppState::InGame)),
        );
        app.add_systems(
            Update,
            sync_script_dialog_state
                .in_set(ScriptChangesApplied)
                .run_if(in_state(AppState::InGame)),
        );
        app.add_systems(
            Update,
            (
                flow::handle_dialog_outputs,
                camera::aim_dialog_camera,
                ui::sync_dialog_ui,
            )
                .chain()
                .after(ScriptChangesApplied)
                .run_if(in_state(AppState::InGame)),
        );
        app.add_systems(
            Update,
            sync_input_mode
                .after(ScriptChangesApplied)
                .after(crate::zengin::gui::ChapterScreenUpdated),
        );
    }
}

/// Exposes the dialog session to the script (`InfoManager_HasFinished`).
fn sync_script_dialog_state(dialog_ui: Res<DialogUiState>, gothic_script: Res<ScriptResource>) {
    let _perf = perf_debug!();
    gothic_script.lock().set_dialog_active(dialog_ui.is_open());
}

/// While editor mode is active in game the input mode is editor, while a
/// dialog, the inventory, the journal or the stats menu is open the input mode
/// is GUI navigation, otherwise gameplay.
pub(crate) fn sync_input_mode(
    state: Res<State<AppState>>,
    editor_mode: Res<EditorMode>,
    dialog_ui: Res<DialogUiState>,
    chapter_screen: Res<crate::zengin::gui::ChapterScreenState>,
    inventory_ui: Res<crate::zengin::gui::InventoryUiState>,
    journal_ui: Res<crate::zengin::gui::JournalUiState>,
    stats_ui: Res<crate::zengin::gui::StatsUiState>,
    mut input_mode: ResMut<InputMode>,
) {
    let _perf = perf_debug!();
    let mode = if *state.get() == AppState::InGame && editor_mode.is_enabled() {
        InputMode::Editor
    } else if *state.get() == AppState::MainMenu
        || chapter_screen.is_open()
        || dialog_ui.is_open()
        || inventory_ui.is_open()
        || journal_ui.is_open()
        || stats_ui.is_open()
    {
        InputMode::Gui
    } else {
        InputMode::Gameplay
    };
    if *input_mode != mode {
        *input_mode = mode;
    }
}
