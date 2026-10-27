//! Host-side storage of daedalus symbol values.
//!
//! Converted script functions no longer keep global variables and class members
//! in a wasm memory. They call the `symbols` interface imports instead, so this
//! store is the only copy of the mutable script state. It also backs the typed
//! projections of well known classes (`C_Npc`, `C_Item`, `C_Menu`, ...) which
//! used to be guest exports reading wasm memory.

use crate::daedalus::Symbol;
use crate::daedalus::SymbolKind;

/// Metadata of one daedalus symbol, mirroring the `symbol-info` guest record.
#[derive(Debug, Clone, Copy)]
pub struct SymbolMeta {
    /// Kind of the symbol.
    pub kind: SymbolKind,
    /// Const symbols are inlined at compile time and have no value slot.
    pub is_const: bool,
    /// Class members live inside their parent instance record, the
    /// `address_or_offset` field is then a byte offset, not a slot base.
    pub is_class_member: bool,
    /// Global symbols: byte base of the value slot. Class members: byte offset
    /// inside the instance record.
    pub address_or_offset: u32,
    /// Whether the symbol has a wasm function with code (instance init code,
    /// daedalus function bodies).
    pub has_function: bool,
}

/// One word space holding every daedalus symbol value, laid out exactly like
/// the old wasm symbol memory: global symbols get a contiguous region, instance
/// records are regions addressed by their class member offsets.
#[derive(Debug, Default, Clone)]
pub struct SymbolStore {
    words: Vec<i32>,
    meta: Vec<SymbolMeta>,
}

impl SymbolStore {
    /// Creates an empty store with the given word capacity.
    #[must_use]
    pub fn new(word_count: usize) -> Self {
        Self {
            words: vec![0; word_count],
            meta: vec![],
        }
    }

    /// Creates a store from parsed daedalus symbols, with their default values
    /// already applied. Slot bases are assigned with the same arithmetic as the
    /// converter's memory layout, tests use this constructor.
    #[must_use]
    pub fn from_symbols(symbols: &[Symbol]) -> Self {
        let mut word_count = 0_u32;
        let mut meta = Vec::with_capacity(symbols.len());
        for symbol in symbols {
            let (is_class_member, address_or_offset) = if symbol.is_class_var {
                (true, symbol.offset.unwrap_or(0))
            } else {
                let address = word_count * 4;
                if !symbol.is_const {
                    word_count += symbol.elements_count;
                }
                (false, address)
            };
            meta.push(SymbolMeta {
                kind: symbol.kind,
                is_const: symbol.is_const,
                is_class_member,
                address_or_offset,
                has_function: symbol.is_function_with_code(),
            });
        }

        let mut store = Self {
            words: vec![0; usize::try_from(word_count).unwrap_or_default()],
            meta,
        };

        for (index, symbol) in symbols.iter().enumerate() {
            if symbol.is_class_var || symbol.is_const {
                continue;
            }
            let base = usize::try_from(store.meta[index].address_or_offset / 4).unwrap_or_default();
            let count = usize::try_from(symbol.elements_count).unwrap_or_default();
            for (element, value) in symbol.default_values.iter().take(count).enumerate() {
                if let Some(word) = store.words.get_mut(base + element) {
                    *word = *value;
                }
            }
        }

        store
    }

    /// Number of symbols the store knows about.
    #[must_use]
    pub fn symbol_count(&self) -> usize {
        self.meta.len()
    }

    /// Number of words in the value space.
    #[must_use]
    pub fn word_count(&self) -> usize {
        self.words.len()
    }

    /// Metadata of a symbol, `None` when the id is out of range.
    #[must_use]
    pub fn meta(&self, symbol: u32) -> Option<&SymbolMeta> {
        self.meta.get(usize::try_from(symbol).ok()?)
    }

    /// Metadata of a symbol, `None` when the id is out of range.
    #[must_use]
    pub fn meta_mut(&mut self, symbol: u32) -> Option<&mut SymbolMeta> {
        self.meta.get_mut(usize::try_from(symbol).ok()?)
    }

    /// Appends metadata loaded from the guest `get-symbol-info` export.
    pub fn push_meta(&mut self, meta: SymbolMeta) {
        self.meta.push(meta);
    }

    /// Writes the initial values of a symbol slot, values longer than the slot
    /// are truncated.
    pub fn set_defaults(&mut self, symbol: u32, values: &[i32]) {
        let Some(base) = self.value_base(symbol) else {
            return;
        };
        for (element, value) in values.iter().enumerate() {
            if let Some(word) = self.words.get_mut(base + element) {
                *word = *value;
            }
        }
    }

    /// Reads element `element` of a global/variable symbol value.
    #[must_use]
    pub fn read_symbol(&self, symbol: u32, element: u32) -> Option<i32> {
        let slot = self.value_slot(symbol, element)?;
        self.words.get(slot).copied()
    }

    /// Writes element `element` of a global/variable symbol value.
    pub fn write_symbol(&mut self, symbol: u32, element: u32, value: i32) -> bool {
        let Some(slot) = self.value_slot(symbol, element) else {
            return false;
        };
        let Some(word) = self.words.get_mut(slot) else {
            return false;
        };
        *word = value;
        true
    }

    /// Reads element `element` of the class member at byte `offset` of
    /// `instance`.
    #[must_use]
    pub fn read_member(&self, instance: u32, offset: u32, element: u32) -> Option<i32> {
        let slot = self.member_slot(instance, offset, element)?;
        self.words.get(slot).copied()
    }

    /// Writes element `element` of the class member at byte `offset` of
    /// `instance`.
    pub fn write_member(&mut self, instance: u32, offset: u32, element: u32, value: i32) -> bool {
        let Some(slot) = self.member_slot(instance, offset, element) else {
            return false;
        };
        let Some(word) = self.words.get_mut(slot) else {
            return false;
        };
        *word = value;
        true
    }

    /// Reads the class member word at byte `offset` of `instance`, used by the
    /// typed projections which keep the offsets of the old guest exports.
    #[must_use]
    pub fn read_offset(&self, instance: u32, offset: u32) -> Option<i32> {
        self.read_member(instance, offset, 0)
    }

    fn value_base(&self, symbol: u32) -> Option<usize> {
        let meta = self.meta(symbol)?;
        if meta.is_class_member {
            return None;
        }
        usize::try_from(meta.address_or_offset / 4).ok()
    }

    fn value_slot(&self, symbol: u32, element: u32) -> Option<usize> {
        let base = self.value_base(symbol)?;
        let element = usize::try_from(element).ok()?;
        Some(base + element)
    }

    fn member_slot(&self, instance: u32, offset: u32, element: u32) -> Option<usize> {
        let meta = self.meta(instance)?;
        if meta.is_class_member {
            return None;
        }
        let base = usize::try_from(meta.address_or_offset / 4).ok()?;
        let offset = usize::try_from(offset / 4).ok()?;
        let element = usize::try_from(element).ok()?;
        Some(base + offset + element)
    }
}

/// Info of an npc instance (`C_Npc` hitpoints, hitpoints max and name).
#[derive(Debug, Default, Clone)]
pub struct NpcInfo {
    /// Current hitpoints (`ATR_HITPOINTS`).
    pub hitpoints: i32,
    /// Maximum hitpoints (`ATR_HITPOINTS_MAX`).
    pub hitpoints_max: i32,
    /// Raw daedalus string id of the name, `None` when unset.
    pub name: Option<i32>,
}

/// Stats of an item instance (`C_Item`).
#[derive(Debug, Default, Clone)]
pub struct ItemInfo {
    /// Total damage.
    pub damage_total: i32,
    /// Raw daedalus string id of the name, `None` when unset.
    pub name: Option<i32>,
    /// Base value.
    pub value: i32,
    /// Weight.
    pub weight: i32,
    /// Equip requirements (attribute ids).
    pub cond_attribute: Vec<i32>,
    /// Equip requirement values.
    pub cond_value: Vec<i32>,
    /// Raw daedalus string id of the description, `None` when unset.
    pub description: Option<i32>,
    /// Info box text string ids.
    pub text: Vec<i32>,
    /// Info box text counts.
    pub count: Vec<i32>,
}

/// Dialog information (`C_Info`).
#[derive(Debug, Default, Clone)]
pub struct DialogInfo {
    /// Npc the info belongs to.
    pub npc: i32,
    /// Order priority.
    pub order: i32,
    /// Condition function symbol id.
    pub condition: i32,
    /// Information function symbol id.
    pub information: i32,
    /// Description string id.
    pub description: i32,
    /// Whether the info is important.
    pub important: bool,
    /// Whether the info is permanent.
    pub permanent: bool,
    /// Whether the info is a trade dialog.
    pub trade: bool,
}

/// A menu (`C_Menu`).
#[derive(Debug, Default, Clone)]
pub struct MenuInfo {
    /// Background picture string id.
    pub back_pic: Option<i32>,
    /// Background world string id.
    pub back_world: Option<i32>,
    /// Position on screen.
    pub pos_x: i32,
    /// Position on screen.
    pub pos_y: i32,
    /// Size on screen.
    pub dim_x: i32,
    /// Size on screen.
    pub dim_y: i32,
    /// Alpha value.
    pub alpha: i32,
    /// Music theme string id.
    pub music_theme: Option<i32>,
    /// Event timer in milliseconds.
    pub event_timer_msec: i32,
    /// Item instance ids.
    pub items: Vec<i32>,
    /// Menu flags.
    pub menu_flags: i32,
    /// Default item outside the game.
    pub default_out_game: i32,
    /// Default item inside the game.
    pub default_in_game: i32,
}

/// A menu item (`C_MenuItem`).
#[derive(Debug, Default, Clone)]
pub struct MenuItemInfo {
    /// Font name string id.
    pub font_name: Option<i32>,
    /// Text string ids.
    pub text: Vec<i32>,
    /// Background picture string id.
    pub back_pic: Option<i32>,
    /// Alpha mode string id.
    pub alpha_mode: Option<i32>,
    /// Alpha value.
    pub alpha: i32,
    /// Item kind.
    pub kind: i32,
    /// Selection action function ids.
    pub on_sel_action: Vec<i32>,
    /// Selection action text string ids.
    pub on_sel_action_s: Vec<i32>,
    /// Set option function string id.
    pub on_chg_set_option: Option<i32>,
    /// Set option section function string id.
    pub on_chg_set_option_section: Option<i32>,
    /// Event action function ids.
    pub on_event_action: Vec<i32>,
    /// Position on screen.
    pub pos_x: i32,
    /// Position on screen.
    pub pos_y: i32,
    /// Size on screen.
    pub dim_x: i32,
    /// Size on screen.
    pub dim_y: i32,
    /// Item flags.
    pub item_flags: i32,
}

/// Projects the `C_Npc` members of an npc instance.
#[must_use]
pub fn npc_info(store: &SymbolStore, instance: u32) -> NpcInfo {
    NpcInfo {
        hitpoints: read_word(store, instance, 440),
        hitpoints_max: read_word(store, instance, 444),
        name: read_string_id(store, instance, 292),
    }
}

/// Projects the `C_Item` members of an item instance.
#[must_use]
pub fn item_info(store: &SymbolStore, instance: u32) -> ItemInfo {
    ItemInfo {
        damage_total: read_word(store, instance, 360),
        name: read_string_id(store, instance, 292),
        value: read_word(store, instance, 352),
        weight: read_word(store, instance, 348),
        cond_attribute: read_list(store, instance, 448, 3),
        cond_value: read_list(store, instance, 460, 3),
        description: read_string_id(store, instance, 628),
        text: read_list(store, instance, 648, 6),
        count: read_list(store, instance, 768, 6),
    }
}

/// Projects the `C_Info` members of a dialog info instance.
#[must_use]
pub fn dialog_info(store: &SymbolStore, instance: u32) -> DialogInfo {
    DialogInfo {
        npc: read_word(store, instance, 0),
        order: read_word(store, instance, 4),
        condition: read_word(store, instance, 12),
        information: read_word(store, instance, 16),
        description: read_word(store, instance, 20),
        important: read_word(store, instance, 8) != 0,
        permanent: read_word(store, instance, 44) != 0,
        trade: read_word(store, instance, 40) != 0,
    }
}

/// Projects the `C_Menu` members of a menu instance (`MENU.DAT` layout).
#[must_use]
pub fn menu_info(store: &SymbolStore, instance: u32) -> MenuInfo {
    MenuInfo {
        back_pic: read_string_id(store, instance, 0),
        back_world: read_string_id(store, instance, 20),
        pos_x: read_word(store, instance, 40),
        pos_y: read_word(store, instance, 44),
        dim_x: read_word(store, instance, 48),
        dim_y: read_word(store, instance, 52),
        alpha: read_word(store, instance, 56),
        music_theme: read_string_id(store, instance, 60),
        event_timer_msec: read_word(store, instance, 80),
        items: read_list(store, instance, 84, 150),
        menu_flags: read_word(store, instance, 3084),
        default_out_game: read_word(store, instance, 3088),
        default_in_game: read_word(store, instance, 3092),
    }
}

/// Projects the `C_MenuItem` members of a menu item instance (`MENU.DAT`
/// layout).
#[must_use]
pub fn menu_item_info(store: &SymbolStore, instance: u32) -> MenuItemInfo {
    MenuItemInfo {
        font_name: read_string_id(store, instance, 0),
        text: read_list(store, instance, 20, 10),
        back_pic: read_string_id(store, instance, 220),
        alpha_mode: read_string_id(store, instance, 240),
        alpha: read_word(store, instance, 260),
        kind: read_word(store, instance, 264),
        on_sel_action: read_list(store, instance, 268, 5),
        on_sel_action_s: read_list(store, instance, 288, 5),
        on_chg_set_option: read_string_id(store, instance, 388),
        on_chg_set_option_section: read_string_id(store, instance, 408),
        on_event_action: read_list(store, instance, 428, 10),
        pos_x: read_word(store, instance, 468),
        pos_y: read_word(store, instance, 472),
        dim_x: read_word(store, instance, 476),
        dim_y: read_word(store, instance, 480),
        item_flags: read_word(store, instance, 488),
    }
}

fn read_word(store: &SymbolStore, instance: u32, offset: u32) -> i32 {
    store.read_offset(instance, offset).unwrap_or_default()
}

fn read_string_id(store: &SymbolStore, instance: u32, offset: u32) -> Option<i32> {
    store
        .read_offset(instance, offset)
        .filter(|value| *value != 0)
}

fn read_list(store: &SymbolStore, instance: u32, base_offset: u32, count: u32) -> Vec<i32> {
    (0..count)
        .map(|element| {
            store
                .read_member(instance, base_offset, element)
                .unwrap_or_default()
        })
        .collect()
}
