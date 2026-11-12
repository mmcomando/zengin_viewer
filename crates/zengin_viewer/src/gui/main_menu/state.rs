use bevy::prelude::*;

use crate::zengin::{
    MENU_ITEM_FLAG_DISABLED, MenuDefinition, MenuItemDefinition, SEL_ACTION_BACK, SEL_ACTION_CLOSE,
    SEL_ACTION_START_MENU,
};

/// Loaded `menu_main` data, `None` when MENU.DAT could not be read.
#[derive(Resource, Default)]
pub(crate) struct MainMenuData(pub Option<MenuDefinition>);

#[derive(Resource, Default)]
pub(crate) struct MainMenuState {
    pub open: bool,
    /// Selected item, identified by its index in the menu's `items[]` array.
    pub selected_slot: usize,
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
            last_size: Vec2::ZERO,
            last_scale: 0.0,
        };
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
        SEL_ACTION_CLOSE => matches!(argument, "NEW_GAME" | "LEAVE_GAME"),
        SEL_ACTION_START_MENU => argument == "MENU_LEAVE_GAME",
        _ => false,
    }
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
