//! Loading of the compiled MENU.DAT script and projection of its `C_MENU` /
//! `C_MENU_ITEM` instance data into UI types.
//!
//! The typed `C_Menu`/`C_MenuItem` models live in `daedalus_typed`, the UI
//! definitions here resolve strings and item instances for rendering.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

use bevy::log::{error, info, warn};
use bevy::prelude::{Resource, World};

use super::manager::{ScriptManager, wasm_cache_path};
use super::state::ScriptMessages;
use super::symbol_id::{FunctionId, StringId};
use crate::zengin::common::gothic2_dir;
use daedalus_typed::{
    MENU_ITEM_FLAG_ONLY_IN_GAME, MENU_ITEM_FLAG_ONLY_OUT_GAME, MENU_ITEM_FLAG_SELECTABLE, Menu,
    MenuItem, SEL_ACTION_UNDEF, ScriptConversion,
};

/// A loaded menu screen with its item instances.
#[derive(Debug, Clone, Default)]
pub(crate) struct MenuDefinition {
    pub back_pic: Option<String>,
    /// `C_Menu.musicTheme` instance name, if the screen defines one.
    pub music_theme: Option<String>,
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
    /// List box inner frame size in menu units.
    pub frame_size: (i32, i32),
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
        self.text.first().map_or_default(String::as_str)
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

        let cache_path = wasm_cache_path("menu.script.wasm");
        let mut script =
            ScriptManager::from_file_cached(&path, &cache_path, ScriptMessages::default());
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

    /// Resolves any menu section by its instance name (for example
    /// `menu_status`) into a [`MenuDefinition`].
    pub fn load_section(&self, section_name: &str) -> Option<MenuDefinition> {
        let mut script = self.lock();
        load_menu(&mut script, section_name)
    }

    /// Locks the menu vm for direct script calls.
    pub fn lock(&self) -> MutexGuard<'_, ScriptManager> {
        self.script.lock().unwrap()
    }
}

/// Returns the shared MENU.DAT vm, creating it when the main menu did not run.
pub(crate) fn ensure_menu_script(world: &mut World) -> Option<MenuScriptResource> {
    if world.get_resource::<MenuScriptResource>().is_none() {
        world.insert_resource(MenuScriptResource::new()?);
    }
    Some((*world.resource::<MenuScriptResource>()).clone())
}

#[expect(
    clippy::cognitive_complexity,
    reason = "Menu loading is a single understandable projection pass"
)]
fn load_menu(script: &mut ScriptManager, section_name: &str) -> Option<MenuDefinition> {
    let Some(section) = script.symbol_instance(section_name) else {
        error!("Menu section {section_name} not found in MENU.DAT");
        return None;
    };
    if !script.is_instance_of_class(section, "c_menu") {
        error!("Menu section {section_name} is not a c_menu instance");
        return None;
    }

    let Some(Menu {
        back_pic,
        music_theme,
        pos_x,
        pos_y,
        dim_x,
        dim_y,
        alpha: alpha_value,
        items: item_ids,
        flags,
        default_out_game,
        default_in_game,
        ..
    }) = script.get_menu_info(section.symbol())
    else {
        error!("Menu section {section_name} has no typed c_menu data");
        return None;
    };

    let items = item_ids
        .iter()
        .enumerate()
        .filter_map(|(slot, name_id)| {
            let name = script.string_by_id(name_id.map(StringId::value))?;
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
            let Some(info) = script.get_menu_item_info(instance.symbol()) else {
                warn!("Menu item {name} has no typed c_menu_item data");
                return None;
            };
            Some(item_definition(slot, name, info, script))
        })
        .collect();

    let definition = MenuDefinition {
        back_pic: non_empty(script.string_by_id(back_pic.map(StringId::value))),
        music_theme: non_empty(script.string_by_id(music_theme.map(StringId::value))),
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
    info: MenuItem,
    script: &mut ScriptManager,
) -> MenuItemDefinition {
    let MenuItem {
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
        frame_size_x,
        frame_size_y,
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
        .map(|symbol| {
            let symbol = symbol?;
            script.symbol_table().function(symbol.value())
        })
        .collect();

    MenuItemDefinition {
        slot,
        name,
        kind: kind.to_i32(),
        text: resolve_strings(script, &text),
        back_pic: non_empty(script.string_by_id(back_pic.map(StringId::value))),
        font_name: non_empty(script.string_by_id(font_name.map(StringId::value))),
        flags,
        pos: (pos_x, pos_y),
        dim: (dim_x, dim_y),
        frame_size: (frame_size_x, frame_size_y),
        alpha: alpha(alpha_value),
        on_sel_action,
        on_event_action,
    }
}

fn resolve_strings(script: &ScriptManager, ids: &[Option<StringId>]) -> Vec<String> {
    ids.iter()
        .map(|id| {
            script
                .string_by_id(id.map(StringId::value))
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
    use daedalus_typed::{MENU_ITEM_LISTBOX, MENU_ITEM_TEXT};

    #[test]
    #[ignore = "requires a Gothic 2 install with MENU.DAT"]
    fn debug_main_menu() {
        let menu = MenuScriptResource::new().expect("MENU.DAT should load");
        let definition = menu.load_main_menu().expect("menu_main should load");
        println!("{definition:#?}");
        assert_eq!(definition.music_theme.as_deref(), Some("SYS_Menu"));
        assert_eq!(definition.default_out_game, 2);
        assert_eq!(definition.default_in_game, 4);
        assert_eq!(definition.items.len(), 10);
        assert_eq!(definition.items[2].name, "MENUITEM_MAIN_NEWGAME");
        assert_eq!(definition.items[2].title(), "Nowa gra");
    }

    #[test]
    #[ignore = "requires a Gothic 2 install with MENU.DAT"]
    fn debug_load_menu() {
        let menu = MenuScriptResource::new().expect("MENU.DAT should load");
        let definition = menu
            .load_section("menu_savegame_load")
            .expect("menu_savegame_load should load");

        assert_eq!(definition.items.len(), 31);
        let item = |name: &str| {
            definition
                .items
                .iter()
                .find(|item| item.name == name)
                .unwrap_or_else(|| panic!("load menu should contain {name}"))
        };
        assert_eq!(item("MENUITEM_LOAD_SLOT1").slot, 9);
        assert_eq!(item("MENUITEM_LOAD_SLOT0").slot, 29);
        assert_eq!(item("MENUITEM_LOAD_BACK").slot, 30);
        assert!(item("MENUITEM_LOAD_SLOT1").is_selectable());
        assert!(item("MENUITEM_LOAD_BACK").is_selectable());
    }

    #[test]
    #[ignore = "requires a Gothic 2 install with MENU.DAT"]
    fn debug_status_menu() {
        let menu = MenuScriptResource::new().expect("MENU.DAT should load");
        let definition = menu
            .load_section("menu_status")
            .expect("menu_status should load");

        assert_eq!(definition.back_pic.as_deref(), Some("status_back.tga"));
        assert_eq!(definition.items.len(), 58);

        let item = |name: &str| {
            definition
                .items
                .iter()
                .find(|item| item.name == name)
                .unwrap_or_else(|| panic!("status menu should contain {name}"))
        };
        assert_eq!(item("MENU_ITEM_ATTRIBUTE_1_TITLE").title(), "Si\u{142}a");
        assert_eq!(item("MENU_ITEM_TALENT_7_CIRCLE").title(), "Kr\u{105}g");
        assert_eq!(item("MENU_ITEM_LEVEL").dim, (-1, -1));
        assert!(item("MENU_ITEM_PLAYERGUILD").title().is_empty());
    }

    #[test]
    #[ignore = "requires a Gothic 2 install with MENU.DAT"]
    fn debug_log_menu() {
        let menu = MenuScriptResource::new().expect("MENU.DAT should load");
        let definition = menu.load_section("menu_log").expect("menu_log should load");

        assert_eq!(definition.back_pic.as_deref(), Some("log_back.tga"));
        assert_eq!(definition.items.len(), 13);

        let item = |name: &str| {
            definition
                .items
                .iter()
                .find(|item| item.name == name)
                .unwrap_or_else(|| panic!("log menu should contain {name}"))
        };
        let list = item("MENU_ITEM_LIST_MISSIONS_ACT");
        assert_eq!(list.kind, MENU_ITEM_LISTBOX);
        println!("log list dim={:?} frame={:?}", list.dim, list.frame_size);
        assert_eq!(item("MENU_ITEM_SEL_LOG").kind, MENU_ITEM_TEXT);
        assert_eq!(item("MENU_ITEM_CONTENT_VIEWER").dim, (8192, 8192));
    }
}
