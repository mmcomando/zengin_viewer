//! Input actions, their entity markers, the active input mode and the input
//! profile loader.

mod options;
mod profile;

pub use options::Options;
pub use profile::load_input_profile;

use bevy::prelude::*;

/// Third person camera switch, carrying the free camera transform the player
/// and camera are placed at.
#[derive(Message)]
pub struct CameraSwitchedToThirdPerson(pub GlobalTransform);
use leafwing_input_manager::prelude::*;
use serde::{Deserialize, Serialize};

/// Which input map is currently active.
///
/// [`GameplayAction`] is enabled in [`InputMode::Gameplay`], [`GuiAction`] in
/// [`InputMode::Gui`] and [`EditorAction`] in [`InputMode::Editor`]. Shared
/// actions are enabled in gameplay and menu modes only.
///
/// [`EditorAction::ToggleEditorMode`] stays enabled in every mode so editor
/// mode can always be toggled.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    #[default]
    Gameplay,
    Gui,
    /// Debugging with the free camera: gameplay and UI input is disabled.
    Editor,
}

/// Marker for the entity holding the gameplay [`InputMap`] (and its required
/// [`ActionState`]).
#[derive(Component)]
pub struct GameplayInputMarker;

/// Marker for the entity holding the GUI navigation [`InputMap`] (and its
/// required [`ActionState`]).
#[derive(Component)]
pub struct GuiInputMarker;

/// Marker for the entity holding the editor [`InputMap`] (and its required
/// [`ActionState`]). Only [`EditorAction::ToggleEditorMode`] is enabled outside
/// [`InputMode::Editor`].
#[derive(Component)]
pub struct EditorInputMarker;

/// Marker for the entity holding the shared [`InputMap`] (and its required
/// [`ActionState`]). Shared input is enabled in the gameplay and GUI modes, but
/// not in [`InputMode::Editor`].
#[derive(Component)]
pub struct SharedInputMarker;

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
    /// Movement direction: forward/backward and, without a forward component,
    /// sideways without turning.
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
    /// One-shot left melee swing, only with a melee weapon drawn.
    AttackLeft,
    /// One-shot right melee swing, only with a melee weapon drawn.
    AttackRight,
    /// Toggle the equipped melee weapon between sheathed and drawn.
    EquipMelee,
    /// Toggle the equipped ranged weapon between sheathed and drawn.
    EquipRanged,
    /// Toggle the last used weapon, falling back to the melee weapon or fists
    /// when no weapon was drawn yet.
    DrawWeapon,
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
    /// Close the current view or skip the currently played content.
    CancelAction,
}

/// Actions available in both gameplay and menu modes, for things like the
/// inventory toggle that must work while a GUI is open. Unlike
/// [`EditorAction`] this set is a normal, mode-independent feature set.
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

/// Actions used while debugging with the free camera, enabled in
/// [`InputMode::Editor`]. [`EditorAction::ToggleEditorMode`] is the exception:
/// it is enabled in every mode so the editor can always be entered and left.
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
pub enum EditorAction {
    TeleportOrigin,
    TeleportAway,
    /// Toggle editor mode, which uses the free camera.
    ToggleEditorMode,
}
