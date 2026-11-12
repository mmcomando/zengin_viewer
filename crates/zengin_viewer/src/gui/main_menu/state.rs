use bevy::prelude::*;

use crate::zengin::{MENU_ITEM_FLAG_DISABLED, MenuDefinition, MenuItemDefinition};

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
    if item.flags & MENU_ITEM_FLAG_DISABLED != 0 {
        return false;
    }
    if item.is_only_in_game() && !in_game {
        return false;
    }
    if item.is_only_out_game() && in_game {
        return false;
    }
    item.is_selectable()
}

/// First selectable item of a menu, used as the initial selection.
pub(super) fn first_selectable(data: &MenuDefinition, in_game: bool) -> Option<usize> {
    data.items
        .iter()
        .find(|item| can_select(item, in_game))
        .map(|item| item.slot)
}

/// The script default item for the current context when it is selectable, the
/// first selectable item otherwise.
pub(super) fn initial_selection(data: &MenuDefinition, in_game: bool) -> Option<usize> {
    let default = if in_game {
        data.default_in_game
    } else {
        data.default_out_game
    };
    is_selectable(data, default, in_game)
        .then_some(default)
        .or_else(|| first_selectable(data, in_game))
}

/// Whether the item with `slot` can be reached with menu navigation.
pub(super) fn is_selectable(data: &MenuDefinition, slot: usize, in_game: bool) -> bool {
    data.items
        .iter()
        .any(|item| item.slot == slot && can_select(item, in_game))
}
