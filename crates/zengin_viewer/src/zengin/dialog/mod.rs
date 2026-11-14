mod camera;
mod flow;
mod important;
mod sounds;
mod state;
mod ui;

pub(crate) use important::ImportantDialogOptions;
pub(crate) use sounds::SpeechSounds;

pub(crate) use state::DialogRequest;
pub(crate) use state::DialogUiState;

use crate::external_messages::pump_external_messages;
use crate::game_state::AppState;
use crate::gameplay::camera::EditorMode;
use crate::input::InputMode;
use crate::zengin::script::bridge::ScriptChangesApplied;
use crate::zengin::script::state::DialogLine;
use bevy::prelude::*;

pub struct GameDialogPlugin;

impl Plugin for GameDialogPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DialogUiState>();
        app.init_resource::<camera::DialogCameraShots>();
        app.add_plugins(important::ImportantDialogsPlugin);
        app.add_message::<DialogRequest>();
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
                flow::show_closest_npc_dialog,
                important::auto_start_important_dialogs
                    .run_if(|options: Res<ImportantDialogOptions>| options.enabled),
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
                .after(crate::gui::ChapterScreenUpdated),
        );
    }
}

/// While editor mode is active in game the input mode is editor, while a
/// dialog, the inventory, the journal or the stats menu is open the input mode
/// is GUI navigation, otherwise gameplay.
pub(crate) fn sync_input_mode(
    state: Res<State<AppState>>,
    editor_mode: Res<EditorMode>,
    dialog_ui: Res<DialogUiState>,
    chapter_screen: Res<crate::gui::ChapterScreenState>,
    inventory_ui: Res<crate::gui::InventoryUiState>,
    journal_ui: Res<crate::gui::JournalUiState>,
    stats_ui: Res<crate::gui::StatsUiState>,
    mut input_mode: ResMut<InputMode>,
) {
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
