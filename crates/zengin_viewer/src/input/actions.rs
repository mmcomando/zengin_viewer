use std::path::Path;

use bevy::prelude::*;
use leafwing_input_manager::plugin::InputManagerSystem;
use leafwing_input_manager::prelude::*;

use input_core::{
    EditorAction, EditorInputMarker, GameplayAction, GameplayInputMarker, GuiAction,
    GuiInputMarker, InputMode, SharedAction, SharedInputMarker, load_input_profile,
};

use crate::input::horizontal_axis::HorizontalAxis;
use crate::input::record::ActionRecordingPlugin;
use crate::input::replay::ActionReplayPlugin;
use dev_util::perf_debug;
use zengin_lib::AppState;
use zengin_lib::gui::{ChapterScreenState, InventoryUiState, JournalUiState, StatsUiState};
use zengin_lib::{DialogRequest, DialogUiState, DialogVoice};

/// Adds [`leafwing_input_manager`] with one input map per action group and
/// spawns the global input entities with the profiles loaded from the assets
/// `input` directory, falling back to the built-in defaults when a file is
/// missing or invalid.
pub struct ActionsPlugin;

impl Plugin for ActionsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InputMode>();
        app.register_dual_axislike_input::<HorizontalAxis>();
        app.add_plugins((
            InputManagerPlugin::<GameplayAction>::default(),
            InputManagerPlugin::<GuiAction>::default(),
            InputManagerPlugin::<SharedAction>::default(),
            InputManagerPlugin::<EditorAction>::default(),
        ));
        app.add_systems(Startup, spawn_input_maps);
        // Mode switching must happen before synthetic replay presses and
        // before consumers read the action states.
        app.add_systems(
            PreUpdate,
            update_input_mode
                .after(InputManagerSystem::Update)
                .before(InputManagerSystem::ManualControl),
        );
        app.add_systems(Update, handle_back);
        app.add_plugins((ActionRecordingPlugin, ActionReplayPlugin));
    }
}

#[allow(clippy::cognitive_complexity, clippy::too_many_arguments)]
fn handle_back(
    state: Res<State<AppState>>,
    action_state: Single<&ActionState<SharedAction>, With<SharedInputMarker>>,
    mut chapter_screen: ResMut<ChapterScreenState>,
    mut inventory_ui: ResMut<InventoryUiState>,
    mut journal_ui: ResMut<JournalUiState>,
    mut stats_ui: ResMut<StatsUiState>,
    mut dialog_ui: ResMut<DialogUiState>,
    mut requests: MessageWriter<DialogRequest>,
    mut voices: MessageWriter<DialogVoice>,
    mut exit: MessageWriter<AppExit>,
) {
    let _perf = perf_debug!();
    if *state.get() == AppState::MainMenu {
        return;
    }
    // Right mouse click is a dialog-only shortcut, it must not close other
    // views or exit the game.
    if action_state.just_pressed(&SharedAction::SkipDialog) {
        if dialog_ui.is_playing_lines() {
            skip_dialog_line(&mut dialog_ui, &mut requests, &mut voices);
        }
        return;
    }
    if !action_state.just_pressed(&SharedAction::Back) {
        return;
    }
    if chapter_screen.is_open() {
        chapter_screen.close();
        info!("Back closed the chapter screen");
        return;
    }
    if inventory_ui.is_open() {
        inventory_ui.close();
        info!("Back closed the open inventory");
        return;
    }
    if journal_ui.is_open() {
        if journal_ui.escape() {
            info!("Back closed the journal entry viewer");
        } else {
            info!("Back closed the open journal");
        }
        return;
    }
    if stats_ui.is_open() {
        stats_ui.close();
        info!("Back closed the open stats menu");
        return;
    }
    if dialog_ui.is_open() {
        if dialog_ui.is_playing_lines() {
            skip_dialog_line(&mut dialog_ui, &mut requests, &mut voices);
        } else {
            dialog_ui.close();
            info!("Back closed the open dialog");
        }
        return;
    }
    info!("Back action triggered, exiting");
    exit.write(AppExit::Success);
}

/// Ends the currently played dialog line early, the next one starts
/// immediately.
fn skip_dialog_line(
    dialog_ui: &mut DialogUiState,
    requests: &mut MessageWriter<DialogRequest>,
    voices: &mut MessageWriter<DialogVoice>,
) {
    dialog_ui.stop_current_line(voices, requests);
    info!("Skipped the playing dialog line");
}

fn spawn_input_maps(mut commands: Commands) {
    let _perf = perf_debug!();
    let assets_dir = Path::new(crate::main_assets_dir());
    let gameplay_map =
        load_input_profile::<GameplayAction>(assets_dir, "input_bindings_gameplay.ron")
            .unwrap_or_else(|error| {
                warn!("Falling back to default gameplay input map: {error}");
                InputMap::default()
            });
    let gui_map = load_input_profile::<GuiAction>(assets_dir, "input_bindings_gui.ron")
        .unwrap_or_else(|error| {
            warn!("Falling back to default GUI input map: {error}");
            InputMap::default()
        });
    let shared_map = load_input_profile::<SharedAction>(assets_dir, "input_bindings_shared.ron")
        .unwrap_or_else(|error| {
            warn!("Falling back to default shared input map: {error}");
            InputMap::default()
        });
    let editor_map = load_input_profile::<EditorAction>(assets_dir, "input_bindings_editor.ron")
        .unwrap_or_else(|error| {
            warn!("Falling back to default editor input map: {error}");
            InputMap::default()
        });
    commands.spawn((GameplayInputMarker, gameplay_map));
    commands.spawn((GuiInputMarker, gui_map));
    commands.spawn((SharedInputMarker, shared_map));
    commands.spawn((EditorInputMarker, editor_map));
}

/// Enables and disables action groups based on [`InputMode`].
///
/// Editor mode disables the shared actions too, so quick save/load, menus and
/// the exit shortcut cannot fire while debugging with the free camera. The
/// editor toggle stays enabled in every mode.
pub(crate) fn update_input_mode(
    input_mode: Res<InputMode>,
    mut gameplay: Single<&mut ActionState<GameplayAction>, With<GameplayInputMarker>>,
    mut gui: Single<&mut ActionState<GuiAction>, With<GuiInputMarker>>,
    mut shared: Single<&mut ActionState<SharedAction>, With<SharedInputMarker>>,
    mut editor: Single<&mut ActionState<EditorAction>, With<EditorInputMarker>>,
) {
    let _perf = perf_debug!();
    match *input_mode {
        InputMode::Gameplay => {
            gui.disable_all_actions();
            gameplay.enable_all_actions();
            shared.enable_all_actions();
            enable_only_editor_toggle(&mut editor);
        }
        InputMode::Gui => {
            gameplay.disable_all_actions();
            gui.enable_all_actions();
            shared.enable_all_actions();
            enable_only_editor_toggle(&mut editor);
        }
        InputMode::Editor => {
            gameplay.disable_all_actions();
            gui.disable_all_actions();
            shared.disable_all_actions();
            editor.enable_all_actions();
        }
    }
}

/// Disables the editor actions except the mode toggle, which must stay
/// available to leave editor mode. The toggle is enabled without resetting it,
/// so a press is not lost when the mode changes on the same frame.
fn enable_only_editor_toggle(editor: &mut ActionState<EditorAction>) {
    editor.disable_action(&EditorAction::TeleportOrigin);
    editor.disable_action(&EditorAction::TeleportAway);
    editor.enable_action(&EditorAction::ToggleEditorMode);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gameplay_profile_parses() {
        let mut app = App::new();
        app.add_plugins(ActionsPlugin);

        if let Err(error) = load_input_profile::<GameplayAction>(
            Path::new(crate::main_assets_dir()),
            "input_bindings_gameplay.ron",
        ) {
            panic!("{error}");
        }
    }
}
