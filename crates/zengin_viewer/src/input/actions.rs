use bevy::prelude::*;
use leafwing_input_manager::plugin::InputManagerSystem;
use leafwing_input_manager::prelude::*;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::audio::{PlaySound, SoundManager};
use crate::game_state::AppState;
use crate::gui::{ChapterScreenState, InventoryUiState, JournalUiState, StatsUiState};
use crate::input::record::ActionRecordingPlugin;
use crate::input::replay::ActionReplayPlugin;
use crate::zengin::{DialogRequest, DialogUiState};

/// Adds [`leafwing_input_manager`] with one input map per action group and
/// spawns the global input entities with the profiles loaded from the RON
/// files in [`INPUT_PROFILE_DIR`], falling back to the built-in defaults when
/// a file is missing or invalid.
pub struct ActionsPlugin;

impl Plugin for ActionsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InputMode>();
        app.add_plugins((
            InputManagerPlugin::<GameplayAction>::default(),
            InputManagerPlugin::<GuiAction>::default(),
            InputManagerPlugin::<SharedAction>::default(),
            InputManagerPlugin::<DebugAction>::default(),
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

/// Which input map is currently active.
///
/// [`DebugAction`] is always enabled; [`GameplayAction`] is enabled in
/// [`InputMode::Gameplay`] and [`GuiAction`] in [`InputMode::Gui`].
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    #[default]
    Gameplay,
    Gui,
}

/// Marker for the entity holding the gameplay [`InputMap`] (and its required
/// [`ActionState`]).
#[derive(Component)]
pub struct GameplayInputMarker;

/// Marker for the entity holding the GUI navigation [`InputMap`] (and its
/// required [`ActionState`]).
#[derive(Component)]
pub struct GuiInputMarker;

/// Marker for the entity holding the debug [`InputMap`] (and its required
/// [`ActionState`]). Debug input is always enabled.
#[derive(Component)]
pub struct DebugInputMarker;

/// Actions controlling the character during gameplay.
///
/// Buttonlike is the default control kind; variants with a different kind are
/// annotated with `#[actionlike(...)]`. `EnumIter` lets the recording iterate
/// over all variants, so new actions are recorded/replayed automatically.
#[derive(
    Actionlike,
    PartialEq,
    Eq,
    Clone,
    Copy,
    Hash,
    Debug,
    Reflect,
    Serialize,
    Deserialize,
    strum::EnumIter,
)]
pub enum GameplayAction {
    /// Movement direction from WASD, the arrow keys, or the left gamepad stick.
    #[actionlike(DualAxis)]
    Move,
    /// Third person camera rotation from the arrow keys or the right gamepad
    /// stick.
    #[actionlike(DualAxis)]
    RotateCamera,
    Jump,
    /// Talk to the selected npc or pick up the selected item.
    Interact,
    /// Deal damage with the drawn weapon to the selected npc.
    DealDamage,
    /// Toggle the equipped melee weapon between sheathed and drawn.
    EquipMelee,
    /// Toggle the equipped ranged weapon between sheathed and drawn.
    EquipRanged,
    /// Move faster while held.
    Accelerate,
}

/// Actions used to navigate menus and other UI.
#[derive(
    Actionlike,
    PartialEq,
    Eq,
    Clone,
    Copy,
    Hash,
    Debug,
    Reflect,
    Serialize,
    Deserialize,
    strum::EnumIter,
)]
pub enum GuiAction {
    /// Move the selection up.
    Up,
    /// Move the selection down.
    Down,
    /// Move the selection left.
    Left,
    /// Move the selection right.
    Right,
    /// Confirm the selected entry.
    Confirm,
    /// Equip/unequip or confirm the selected inventory entry. Uses the
    /// gameplay Interact keys (left mouse button, left ctrl).
    Interact,
    /// Close the current view or skip the currently played content.
    CancelAction,
}

/// Actions available in both [`InputMode`]s (gameplay and menu), for things
/// like the inventory toggle that must work while a GUI is open. Unlike
/// [`DebugAction`] this set is a normal, mode-independent feature set.
#[derive(
    Actionlike,
    PartialEq,
    Eq,
    Clone,
    Copy,
    Hash,
    Debug,
    Reflect,
    Serialize,
    Deserialize,
    strum::EnumIter,
)]
pub enum SharedAction {
    /// Toggle the player inventory UI, closes it when it is open.
    Inventory,
    /// Toggle the journal UI, closes it when it is open.
    Journal,
    /// Toggle the player stats (character sheet) UI, closes it when it is open.
    Stats,
    /// Write a new timestamped save.
    QuickSave,
    /// Load the newest save.
    QuickLoad,
    /// Closes the topmost open UI, otherwise exits the game.
    Back,
    /// Skips the dialog line currently played back. Does not close the dialog
    /// while it waits for an option.
    SkipDialog,
}

/// Marker for the entity holding the shared [`InputMap`] (and its required
/// [`ActionState`]). Shared input is enabled in both gameplay and GUI modes.
#[derive(Component)]
pub struct SharedInputMarker;

/// Debug helper actions, always available regardless of [`InputMode`].
#[derive(
    Actionlike,
    PartialEq,
    Eq,
    Clone,
    Copy,
    Hash,
    Debug,
    Reflect,
    Serialize,
    Deserialize,
    strum::EnumIter,
)]
pub enum DebugAction {
    TeleportOrigin,
    TeleportAway,
    /// Toggle between the free camera and the character camera.
    ToggleCharacterCamera,
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
    mut sounds: MessageWriter<PlaySound>,
    mut sound_manager: ResMut<SoundManager>,
    mut exit: MessageWriter<AppExit>,
    mut commands: Commands,
) {
    if *state.get() == AppState::MainMenu {
        return;
    }
    // Right mouse click is a dialog-only shortcut, it must not close other
    // views or exit the game.
    if action_state.just_pressed(&SharedAction::SkipDialog) {
        if dialog_ui.is_playing_lines() {
            skip_dialog_line(
                &mut dialog_ui,
                &mut requests,
                &mut sounds,
                &mut sound_manager,
                &mut commands,
            );
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
            skip_dialog_line(
                &mut dialog_ui,
                &mut requests,
                &mut sounds,
                &mut sound_manager,
                &mut commands,
            );
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
    sounds: &mut MessageWriter<PlaySound>,
    sound_manager: &mut SoundManager,
    commands: &mut Commands,
) {
    dialog_ui.stop_current_line(sounds, requests);
    sound_manager.stop_active(commands);
    info!("Skipped the playing dialog line");
}

fn spawn_input_maps(mut commands: Commands) {
    let gameplay_map = load_input_profile::<GameplayAction>("input_bindings_gameplay.ron")
        .unwrap_or_else(|error| {
            warn!("Falling back to default gameplay input map: {error}");
            InputMap::default()
        });
    let gui_map =
        load_input_profile::<GuiAction>("input_bindings_gui.ron").unwrap_or_else(|error| {
            warn!("Falling back to default GUI input map: {error}");
            InputMap::default()
        });
    let shared_map = load_input_profile::<SharedAction>("input_bindings_shared.ron")
        .unwrap_or_else(|error| {
            warn!("Falling back to default shared input map: {error}");
            InputMap::default()
        });
    let debug_map =
        load_input_profile::<DebugAction>("input_bindings_debug.ron").unwrap_or_else(|error| {
            warn!("Falling back to default debug input map: {error}");
            InputMap::default()
        });
    commands.spawn((GameplayInputMarker, gameplay_map));
    commands.spawn((GuiInputMarker, gui_map));
    commands.spawn((SharedInputMarker, shared_map));
    commands.spawn((DebugInputMarker, debug_map));
}

/// Enables and disables action groups based on [`InputMode`].
///
/// The shared and debug groups are never touched, so they stay enabled in
/// every mode.
pub(crate) fn update_input_mode(
    input_mode: Res<InputMode>,
    mut gameplay: Single<&mut ActionState<GameplayAction>, With<GameplayInputMarker>>,
    mut gui: Single<&mut ActionState<GuiAction>, With<GuiInputMarker>>,
) {
    match *input_mode {
        InputMode::Gameplay => {
            gui.disable_all_actions();
            gameplay.enable_all_actions();
        }
        InputMode::Gui => {
            gameplay.disable_all_actions();
            gui.enable_all_actions();
        }
    }
}

/// Loads an input profile from the `input` subdirectory of the main assets
/// directory.
fn load_input_profile<A>(file_name: &str) -> Result<InputMap<A>, String>
where
    A: Actionlike + Serialize + DeserializeOwned,
{
    let path = format!("{}/input/{file_name}", crate::main_assets_dir());
    let contents = std::fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {path}: {error}"))?;
    ron::de::from_str(&contents).map_err(|error| format!("failed to parse {path}: {error}"))
}
