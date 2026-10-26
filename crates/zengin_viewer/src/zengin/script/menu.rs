//! Loading of the compiled MENU.DAT script and projection of its `C_MENU` /
//! `C_MENU_ITEM` instance data into Rust types.
//!
//! The guest's `manage` interface owns the daedalus class offsets, the host
//! only receives the projected menu records.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

use bevy::log::{error, info, warn};
use bevy::prelude::Resource;

use super::bindings::exports::mmcomando::zengine_scripts::manage::{MenuInfo, MenuItemInfo};
use super::manager::ScriptManager;
use super::symbol_id::{FunctionId, SymbolId};
use crate::zengin::common::gothic2_dir;

/// Engine select-action command ids, see the `SEL_ACTION_*` constants of the
/// menu script.
pub(crate) const SEL_ACTION_UNDEF: i32 = 0;
pub(crate) const SEL_ACTION_BACK: i32 = 1;
pub(crate) const SEL_ACTION_START_MENU: i32 = 2;
pub(crate) const SEL_ACTION_START_ITEM: i32 = 3;
pub(crate) const SEL_ACTION_CLOSE: i32 = 4;
pub(crate) const SEL_ACTION_CON_COMMANDS: i32 = 5;
pub(crate) const SEL_ACTION_PLAY_SOUND: i32 = 6;
pub(crate) const SEL_ACTION_EXEC_COMMANDS: i32 = 7;

/// Engine menu item type ids, see the `MENU_ITEM_*` constants of the menu
/// script.
pub(crate) const MENU_ITEM_UNDEF: i32 = 0;
pub(crate) const MENU_ITEM_TEXT: i32 = 1;
pub(crate) const MENU_ITEM_SLIDER: i32 = 2;
pub(crate) const MENU_ITEM_INPUT: i32 = 3;
pub(crate) const MENU_ITEM_CURSOR: i32 = 4;
pub(crate) const MENU_ITEM_CHOICEBOX: i32 = 5;
pub(crate) const MENU_ITEM_BUTTON: i32 = 6;
pub(crate) const MENU_ITEM_LISTBOX: i32 = 7;

/// Menu item flags, see the `IT_*` constants of the menu script.
pub(crate) const MENU_ITEM_FLAG_CHROMAKEYED: i32 = 1;
pub(crate) const MENU_ITEM_FLAG_TRANSPARENT: i32 = 2;
pub(crate) const MENU_ITEM_FLAG_SELECTABLE: i32 = 4;
pub(crate) const MENU_ITEM_FLAG_MOVEABLE: i32 = 8;
pub(crate) const MENU_ITEM_FLAG_TXT_CENTER: i32 = 16;
pub(crate) const MENU_ITEM_FLAG_DISABLED: i32 = 32;
pub(crate) const MENU_ITEM_FLAG_FADE: i32 = 64;
pub(crate) const MENU_ITEM_FLAG_EFFECTS_NEXT: i32 = 128;
pub(crate) const MENU_ITEM_FLAG_ONLY_OUT_GAME: i32 = 256;
pub(crate) const MENU_ITEM_FLAG_ONLY_IN_GAME: i32 = 512;
pub(crate) const MENU_ITEM_FLAG_PERF_OPTION: i32 = 1 << 10;
pub(crate) const MENU_ITEM_FLAG_MULTILINE: i32 = 1 << 11;
pub(crate) const MENU_ITEM_FLAG_NEEDS_APPLY: i32 = 1 << 12;
pub(crate) const MENU_ITEM_FLAG_NEEDS_RESTART: i32 = 1 << 13;
pub(crate) const MENU_ITEM_FLAG_EXTENDED_MENU: i32 = 1 << 14;

/// Menu screen flags, see the `MENU_*` constants of the menu script.
pub(crate) const MENU_FLAG_OVERTOP: i32 = 1;
pub(crate) const MENU_FLAG_EXCLUSIVE: i32 = 2;
pub(crate) const MENU_FLAG_NO_ANI: i32 = 4;
pub(crate) const MENU_FLAG_DONT_SCALE_DIM: i32 = 8;
pub(crate) const MENU_FLAG_DONT_SCALE_POS: i32 = 16;
pub(crate) const MENU_FLAG_ALIGN_CENTER: i32 = 32;
pub(crate) const MENU_FLAG_SHOW_INFO: i32 = 64;

/// Menu item select event ids, see the `EVENT_*` constants of the menu script.
pub(crate) const MENU_EVENT_EXECUTE: usize = 1;

/// Position and size are expressed in this many units per screen dimension.
pub(crate) const MENU_SCRIPT_DIV: f32 = 8192.0;

/// A loaded menu screen with its item instances.
#[derive(Debug, Clone, Default)]
pub(crate) struct MenuDefinition {
    pub back_pic: Option<String>,
    pub pos: (i32, i32),
    pub dim: (i32, i32),
    pub alpha: u8,
    pub flags: i32,
    /// Index into the source `items[]` array, see [`MenuItemDefinition::slot`].
    pub default_out_game: usize,
    /// Index into the source `items[]` array, see [`MenuItemDefinition::slot`].
    pub default_in_game: usize,
    pub items: Vec<MenuItemDefinition>,
}

/// A loaded menu item instance referenced by a menu screen.
#[derive(Debug, Clone, Default)]
pub(crate) struct MenuItemDefinition {
    /// Index of the item in the menu screen's `items[]` array.
    pub slot: usize,
    pub name: String,
    pub kind: i32,
    /// Full `text[]` array; unset entries are empty strings.
    pub text: Vec<String>,
    pub back_pic: Option<String>,
    pub font_name: Option<String>,
    pub flags: i32,
    pub pos: (i32, i32),
    pub dim: (i32, i32),
    pub alpha: u8,
    /// Non-`SEL_ACTION_UNDEF` entries with their matching `onSelAction_S` text.
    pub on_sel_action: Vec<(i32, String)>,
    /// Full `onEventAction[]` array indexed by `MENU_EVENT_*`.
    pub on_event_action: Vec<Option<FunctionId>>,
}

impl MenuItemDefinition {
    pub fn is_selectable(&self) -> bool {
        self.flags & MENU_ITEM_FLAG_SELECTABLE != 0
    }

    pub fn is_only_in_game(&self) -> bool {
        self.flags & MENU_ITEM_FLAG_ONLY_IN_GAME != 0
    }

    pub fn is_only_out_game(&self) -> bool {
        self.flags & MENU_ITEM_FLAG_ONLY_OUT_GAME != 0
    }

    pub fn title(&self) -> &str {
        self.text.first().map(String::as_str).unwrap_or_default()
    }

    /// Wrapped text of the original menu scripts (`text[0]` and continuation
    /// lines).
    pub fn texts(&self) -> impl Iterator<Item = &str> {
        self.text
            .iter()
            .map(String::as_str)
            .skip(1)
            .take_while(|text| !text.is_empty())
    }
}

/// MENU.DAT script vm.
///
/// Like [`ScriptResource`](super::component::ScriptResource), clones share the
/// same vm and access goes through [`MenuScriptResource::lock`].
#[derive(Resource, Clone)]
pub(crate) struct MenuScriptResource {
    script: Arc<Mutex<ScriptManager>>,
}

impl MenuScriptResource {
    /// Loads MENU.DAT, `None` when the file is missing.
    pub fn new() -> Option<Self> {
        let path = PathBuf::from(format!(
            "{}/_work/Data/Scripts/_compiled/MENU.DAT",
            gothic2_dir()
        ));
        if !path.is_file() {
            error!("Menu script {} is missing", path.display());
            return None;
        }

        let cache_path = PathBuf::from("./menu.script.wasm");
        let mut script = ScriptManager::from_file_cached(&path, &cache_path);
        script.load_symbol_table();
        script.initialize_instances();
        info!("Loaded menu script {}", path.display());
        Some(Self {
            script: Arc::new(Mutex::new(script)),
        })
    }

    /// Resolves `menu_main` into a [`MenuDefinition`].
    pub fn load_main_menu(&self) -> Option<MenuDefinition> {
        let mut script = self.lock();
        load_menu(&mut script, "menu_main")
    }

    /// Locks the menu vm for direct script calls.
    pub fn lock(&self) -> MutexGuard<'_, ScriptManager> {
        self.script.lock().unwrap()
    }
}

fn load_menu(script: &mut ScriptManager, section_name: &str) -> Option<MenuDefinition> {
    let Some(section) = script.symbol_instance(section_name) else {
        error!("Menu section {section_name} not found in MENU.DAT");
        return None;
    };
    if !script.is_instance_of_class(section, "c_menu") {
        error!("Menu section {section_name} is not a c_menu instance");
        return None;
    }

    let MenuInfo {
        back_pic,
        pos_x,
        pos_y,
        dim_x,
        dim_y,
        alpha: alpha_value,
        items: item_ids,
        menu_flags: flags,
        default_out_game,
        default_in_game,
        ..
    } = script.get_menu_info(section.symbol());

    let items = item_ids
        .into_iter()
        .enumerate()
        .filter_map(|(slot, name_id)| {
            let name = script.string_by_id((name_id != 0).then_some(name_id))?;
            if name.is_empty() {
                return None;
            }
            let Some(instance) = script.symbol_instance(&name.to_lowercase()) else {
                warn!("Menu item {name} not found in MENU.DAT");
                return None;
            };
            if !script.is_instance_of_class(instance, "c_menu_item") {
                warn!("Menu item {name} is not a c_menu_item instance");
                return None;
            }
            let info = script.get_menu_item_info(instance.symbol());
            Some(item_definition(slot, name, info, script))
        })
        .collect();

    let definition = MenuDefinition {
        back_pic: non_empty(script.string_by_id(back_pic)),
        pos: (pos_x, pos_y),
        dim: (dim_x, dim_y),
        alpha: alpha(alpha_value),
        flags,
        default_out_game: slot(default_out_game),
        default_in_game: slot(default_in_game),
        items,
    };
    info!(
        "Loaded menu section {section_name} with {} item(s)",
        definition.items.len()
    );
    Some(definition)
}

fn item_definition(
    slot: usize,
    name: String,
    info: MenuItemInfo,
    script: &mut ScriptManager,
) -> MenuItemDefinition {
    let MenuItemInfo {
        font_name,
        text,
        back_pic,
        alpha: alpha_value,
        kind,
        on_sel_action,
        on_sel_action_s,
        on_event_action,
        pos_x,
        pos_y,
        dim_x,
        dim_y,
        item_flags: flags,
        ..
    } = info;

    let on_sel_action_s = resolve_strings(script, &on_sel_action_s);
    let on_sel_action = on_sel_action
        .into_iter()
        .enumerate()
        .filter(|(_, action)| *action != SEL_ACTION_UNDEF)
        .map(|(index, action)| {
            (
                action,
                on_sel_action_s.get(index).cloned().unwrap_or_default(),
            )
        })
        .collect();
    let on_event_action = on_event_action
        .into_iter()
        .map(|raw| {
            let symbol = SymbolId::new(raw)?;
            script.symbol_table().function(symbol.value())
        })
        .collect();

    MenuItemDefinition {
        slot,
        name,
        kind,
        text: resolve_strings(script, &text),
        back_pic: non_empty(script.string_by_id(back_pic)),
        font_name: non_empty(script.string_by_id(font_name)),
        flags,
        pos: (pos_x, pos_y),
        dim: (dim_x, dim_y),
        alpha: alpha(alpha_value),
        on_sel_action,
        on_event_action,
    }
}

fn resolve_strings(script: &ScriptManager, ids: &[i32]) -> Vec<String> {
    ids.iter()
        .map(|id| {
            script
                .string_by_id((*id != 0).then_some(*id))
                .unwrap_or_default()
        })
        .collect()
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.is_empty())
}

fn alpha(value: i32) -> u8 {
    u8::try_from(value.clamp(0, 255)).unwrap_or(255)
}

fn slot(value: i32) -> usize {
    usize::try_from(value.max(0)).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires a Gothic 2 install with MENU.DAT"]
    fn debug_main_menu() {
        let menu = MenuScriptResource::new().expect("MENU.DAT should load");
        let definition = menu.load_main_menu().expect("menu_main should load");
        println!("{definition:#?}");
        assert_eq!(definition.default_out_game, 2);
        assert_eq!(definition.default_in_game, 4);
        assert_eq!(definition.items.len(), 10);
        assert_eq!(definition.items[2].name, "MENUITEM_MAIN_NEWGAME");
        assert_eq!(definition.items[2].title(), "Nowa gra");
    }
}
