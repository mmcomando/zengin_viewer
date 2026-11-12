use std::path::PathBuf;

use bevy::prelude::*;

use crate::zengin::{
    MENU_ITEM_FLAG_DISABLED, MenuDefinition, MenuItemDefinition, SEL_ACTION_BACK, SEL_ACTION_CLOSE,
    SEL_ACTION_START_MENU,
};

/// Loaded `menu_main` data, `None` when MENU.DAT could not be read.
#[derive(Resource, Default)]
pub(crate) struct MainMenuData(pub Option<MenuDefinition>);

/// Loaded `menu_savegame_load` data, `None` when it could not be projected.
#[derive(Resource, Default)]
pub(crate) struct LoadMenuData(pub Option<MenuDefinition>);

/// Which MENU.DAT screen the main menu is currently showing.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MainMenuScreen {
    #[default]
    Main,
    Load,
}

#[derive(Resource, Default)]
pub(crate) struct MainMenuState {
    pub open: bool,
    pub screen: MainMenuScreen,
    /// Selected item, identified by its index in the menu's `items[]` array.
    pub selected_slot: usize,
    /// Save files shown by the load screen, newest first.
    pub saved_games: Vec<PathBuf>,
    pub rebuild: bool,
    pub last_size: Vec2,
    pub last_scale: f32,
}

impl MainMenuState {
    pub fn open(&mut self, selected_slot: usize) {
        *self = Self {
            open: true,
            selected_slot,
            rebuild: true,
            ..Self::default()
        };
    }

    pub fn open_load_screen(&mut self, selected_slot: usize) {
        self.screen = MainMenuScreen::Load;
        self.saved_games = crate::save::list_saves();
        self.selected_slot = selected_slot;
        self.rebuild = true;
    }

    pub fn open_main_screen(&mut self, selected_slot: usize) {
        self.screen = MainMenuScreen::Main;
        self.saved_games.clear();
        self.selected_slot = selected_slot;
        self.rebuild = true;
    }

    pub fn close(&mut self) {
        *self = Self::default();
    }

    pub fn selected_item<'a>(&self, data: &'a MenuDefinition) -> Option<&'a MenuItemDefinition> {
        data.items
            .iter()
            .find(|item| item.slot == self.selected_slot)
    }
}

/// Whether an item can be reached with menu navigation. `in_game` is true when
/// a game world is loaded, enabling `IT_ONLY_IN_GAME` items (e.g. resume) and
/// disabling the out-of-game ones.
pub(super) fn can_select(item: &MenuItemDefinition, in_game: bool) -> bool {
    item.is_selectable() && !is_disabled(item, in_game)
}

/// Whether an item should be shown disabled and skipped during navigation: it
/// is hidden in the current context or its selection action is not implemented
/// yet.
pub(super) fn is_disabled(item: &MenuItemDefinition, in_game: bool) -> bool {
    if item.flags & MENU_ITEM_FLAG_DISABLED != 0 {
        return true;
    }
    if item.is_only_in_game() && !in_game {
        return true;
    }
    if item.is_only_out_game() && in_game {
        return true;
    }
    !has_implemented_action(item)
}

/// Whether selecting the item runs at least one implemented action and no
/// unimplemented one, matching the dispatch in `input::apply_action`.
fn has_implemented_action(item: &MenuItemDefinition) -> bool {
    if item.on_event_action.iter().any(Option::is_some) {
        return false;
    }
    !item.on_sel_action.is_empty()
        && item
            .on_sel_action
            .iter()
            .all(|(action, argument)| action_is_implemented(*action, argument))
}

/// Whether `input::apply_action` handles the given selection action.
fn action_is_implemented(action: i32, argument: &str) -> bool {
    match action {
        SEL_ACTION_BACK => true,
        SEL_ACTION_CLOSE => matches!(argument, "NEW_GAME" | "LEAVE_GAME" | "SAVEGAME_LOAD"),
        SEL_ACTION_START_MENU => matches!(
            argument,
            "MENU_LEAVE_GAME" | "MENU_SAVEGAME_SAVE" | "MENU_SAVEGAME_LOAD"
        ),
        _ => false,
    }
}

/// Slots reachable with Up/Down on the main menu.
pub(super) fn selectable_slots(data: &MenuDefinition, in_game: bool) -> Vec<usize> {
    data.items
        .iter()
        .filter(|item| can_select(item, in_game))
        .map(|item| item.slot)
        .collect()
}

/// MENU.DAT load-slot row names (`MENUITEM_LOAD_SLOT1`, ..., `MENUITEM_LOAD_SLOT0`).
const LOAD_SLOT_PREFIX: &str = "MENUITEM_LOAD_SLOT";
/// The load screen row that returns to the main menu.
const LOAD_BACK_NAME: &str = "MENUITEM_LOAD_BACK";

/// Item slots of the load screen's save rows in display order. Index `i` in
/// this list corresponds to save entry `i`.
pub(super) fn load_slot_slots(data: &MenuDefinition) -> Vec<usize> {
    data.items
        .iter()
        .filter(|item| item.name.starts_with(LOAD_SLOT_PREFIX))
        .map(|item| item.slot)
        .collect()
}

/// Position of a load-screen item slot in [`load_slot_slots`].
pub(super) fn load_slot_index(data: &MenuDefinition, slot: usize) -> Option<usize> {
    load_slot_slots(data)
        .iter()
        .position(|candidate| *candidate == slot)
}

/// Whether the item is the load screen's back row. The script leaves it without
/// an action, so it is dispatched specially.
pub(super) fn is_load_back(item: &MenuItemDefinition) -> bool {
    item.name == LOAD_BACK_NAME
}

/// Slots reachable with Up/Down on the load screen: rows backed by a save file
/// plus the back row.
pub(super) fn load_selectable_slots(data: &MenuDefinition, saved: &[PathBuf]) -> Vec<usize> {
    let mut slots: Vec<usize> = load_slot_slots(data)
        .into_iter()
        .enumerate()
        .filter(|(index, _)| saved.get(*index).is_some())
        .map(|(_, slot)| slot)
        .collect();
    if let Some(back) = data.items.iter().find(|item| is_load_back(item)) {
        slots.push(back.slot);
    }
    slots
}

/// First selectable item of a menu, used as the initial selection.
pub(super) fn first_selectable(data: &MenuDefinition, in_game: bool) -> Option<usize> {
    data.items
        .iter()
        .find(|item| can_select(item, in_game))
        .map(|item| item.slot)
}

/// The script default item for the current context when selectable, otherwise
/// the nearest selectable item at or after it.
pub(super) fn initial_selection(data: &MenuDefinition, in_game: bool) -> Option<usize> {
    let default = if in_game {
        data.default_in_game
    } else {
        data.default_out_game
    };
    if is_selectable(data, default, in_game) {
        return Some(default);
    }
    // The script default may be an unimplemented item (e.g. save); land on the
    // next usable item instead of the first one.
    data.items
        .iter()
        .filter(|item| item.slot >= default)
        .find(|item| can_select(item, in_game))
        .map(|item| item.slot)
        .or_else(|| first_selectable(data, in_game))
}

/// Whether the item with `slot` can be reached with menu navigation.
pub(super) fn is_selectable(data: &MenuDefinition, slot: usize, in_game: bool) -> bool {
    data.items
        .iter()
        .any(|item| item.slot == slot && can_select(item, in_game))
}
