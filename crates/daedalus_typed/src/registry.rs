//! Typed script-class registry and VM access dispatch.

use std::collections::HashMap;
use std::sync::{Arc, PoisonError, RwLock};

use super::access::ScriptVmAccess;
use super::generic::GenericVariable;
use super::ids::StringId;
use super::info::Info;
use super::item::Item;
use super::menu::Menu;
use super::menu_item::MenuItem;
use super::npc::Npc;
use super::symbol::{SymbolDecl, SymbolKind};

/// One modeled field slot used for load-time validation and value routing.
#[derive(Debug, Clone, Copy)]
pub(crate) struct FieldSlot {
    /// Daedalus member name inside the class (without the `c_npc.` prefix).
    pub member: &'static str,
    /// First modeled element.
    pub element: u32,
    /// Byte offset of the member inside the instance record.
    pub offset: u32,
    /// Number of modeled elements.
    pub count: u32,
}

/// Script classes with a typed host model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassKind {
    /// `C_Npc`.
    Npc,
    /// `C_Item`.
    Item,
    /// `C_Menu` (MENU.DAT).
    Menu,
    /// `C_MenuItem` (MENU.DAT).
    MenuItem,
    /// `C_Info` dialog information.
    Info,
}

const CLASS_KINDS: [ClassKind; 5] = [
    ClassKind::Npc,
    ClassKind::Item,
    ClassKind::Menu,
    ClassKind::MenuItem,
    ClassKind::Info,
];

impl ClassKind {
    /// Typed class matching a daedalus class name, case-insensitive.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        if name.eq_ignore_ascii_case("c_npc") {
            return Some(Self::Npc);
        }
        if name.eq_ignore_ascii_case("c_item") {
            return Some(Self::Item);
        }
        if name.eq_ignore_ascii_case("c_menu") {
            return Some(Self::Menu);
        }
        if name.eq_ignore_ascii_case("c_menu_item") {
            return Some(Self::MenuItem);
        }
        if name.eq_ignore_ascii_case("c_info") {
            return Some(Self::Info);
        }
        None
    }

    /// Daedalus class name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Npc => "c_npc",
            Self::Item => "c_item",
            Self::Menu => "c_menu",
            Self::MenuItem => "c_menu_item",
            Self::Info => "c_info",
        }
    }

    fn index(self) -> usize {
        match self {
            Self::Npc => 0,
            Self::Item => 1,
            Self::Menu => 2,
            Self::MenuItem => 3,
            Self::Info => 4,
        }
    }

    fn field_slots(self) -> &'static [FieldSlot] {
        match self {
            Self::Npc => super::npc::FIELD_SLOTS,
            Self::Item => super::item::FIELD_SLOTS,
            Self::Menu => super::menu::FIELD_SLOTS,
            Self::MenuItem => super::menu_item::FIELD_SLOTS,
            Self::Info => super::info::FIELD_SLOTS,
        }
    }

    /// Whether word 0 of an instance record is the instance reference instead of
    /// a modeled member. `C_Npc`/`C_Item` have a 288 byte object header before
    /// their members.
    fn uses_reference_word(self) -> bool {
        matches!(self, Self::Npc | Self::Item)
    }
}

/// Value storage of one daedalus symbol.
#[derive(Debug, Clone)]
pub enum TypedInstance {
    /// `C_Npc` instance.
    Npc(Arc<RwLock<Npc>>),
    /// `C_Item` instance.
    Item(Arc<RwLock<Item>>),
    /// `C_Menu` instance.
    Menu(Arc<RwLock<Menu>>),
    /// `C_MenuItem` instance.
    MenuItem(Arc<RwLock<MenuItem>>),
    /// `C_Info` instance.
    Info(Arc<RwLock<Info>>),
    /// Symbol value without a typed class model.
    Generic(GenericVariable),
}

impl TypedInstance {
    /// Typed class of the stored instance, `None` for [`Self::Generic`].
    #[must_use]
    pub fn class(&self) -> Option<ClassKind> {
        match self {
            Self::Npc(_) => Some(ClassKind::Npc),
            Self::Item(_) => Some(ClassKind::Item),
            Self::Menu(_) => Some(ClassKind::Menu),
            Self::MenuItem(_) => Some(ClassKind::MenuItem),
            Self::Info(_) => Some(ClassKind::Info),
            Self::Generic(_) => None,
        }
    }

    /// Npc handle, `None` for other classes.
    #[must_use]
    pub fn npc(&self) -> Option<&Arc<RwLock<Npc>>> {
        match self {
            Self::Npc(handle) => Some(handle),
            _ => None,
        }
    }

    /// Item handle, `None` for other classes.
    #[must_use]
    pub fn item(&self) -> Option<&Arc<RwLock<Item>>> {
        match self {
            Self::Item(handle) => Some(handle),
            _ => None,
        }
    }

    /// Menu handle, `None` for other classes.
    #[must_use]
    pub fn menu(&self) -> Option<&Arc<RwLock<Menu>>> {
        match self {
            Self::Menu(handle) => Some(handle),
            _ => None,
        }
    }

    /// Menu item handle, `None` for other classes.
    #[must_use]
    pub fn menu_item(&self) -> Option<&Arc<RwLock<MenuItem>>> {
        match self {
            Self::MenuItem(handle) => Some(handle),
            _ => None,
        }
    }

    /// Dialog info handle, `None` for other classes.
    #[must_use]
    pub fn info(&self) -> Option<&Arc<RwLock<Info>>> {
        match self {
            Self::Info(handle) => Some(handle),
            _ => None,
        }
    }

    /// Generic value storage, `None` for typed class instances.
    #[must_use]
    pub fn generic(&self) -> Option<&GenericVariable> {
        match self {
            Self::Generic(variable) => Some(variable),
            _ => None,
        }
    }

    /// Mutable generic value storage, `None` for typed class instances.
    pub fn generic_mut(&mut self) -> Option<&mut GenericVariable> {
        match self {
            Self::Generic(variable) => Some(variable),
            _ => None,
        }
    }

    fn read(&self, offset: u32, element: u32) -> Option<i32> {
        match self {
            Self::Npc(handle) => read_lock(handle).read(offset, element),
            Self::Item(handle) => read_lock(handle).read(offset, element),
            Self::Menu(handle) => read_lock(handle).read(offset, element),
            Self::MenuItem(handle) => read_lock(handle).read(offset, element),
            Self::Info(handle) => read_lock(handle).read(offset, element),
            Self::Generic(variable) => variable.get(offset / 4 + element),
        }
    }

    fn write(&mut self, offset: u32, element: u32, value: i32) -> bool {
        match self {
            Self::Npc(handle) => write_lock(handle).write(offset, element, value),
            Self::Item(handle) => write_lock(handle).write(offset, element, value),
            Self::Menu(handle) => write_lock(handle).write(offset, element, value),
            Self::MenuItem(handle) => write_lock(handle).write(offset, element, value),
            Self::Info(handle) => write_lock(handle).write(offset, element, value),
            Self::Generic(variable) => variable.set(offset / 4 + element, value),
        }
    }

    /// Independent copy of the stored record, typed class handles get a new lock
    /// and a cloned payload.
    fn deep_copy(&self) -> Option<Self> {
        match self {
            Self::Npc(handle) => Some(Self::Npc(Arc::new(RwLock::new(read_lock(handle).clone())))),
            Self::Item(handle) => {
                Some(Self::Item(Arc::new(RwLock::new(read_lock(handle).clone()))))
            }
            Self::Menu(handle) => {
                Some(Self::Menu(Arc::new(RwLock::new(read_lock(handle).clone()))))
            }
            Self::MenuItem(handle) => Some(Self::MenuItem(Arc::new(RwLock::new(
                read_lock(handle).clone(),
            )))),
            Self::Info(handle) => {
                Some(Self::Info(Arc::new(RwLock::new(read_lock(handle).clone()))))
            }
            Self::Generic(variable) => Some(Self::Generic(variable.clone())),
        }
    }
}

#[derive(Debug, Clone)]
struct SymbolEntry {
    name: String,
    kind: SymbolKind,
    is_const: bool,
    is_class_member: bool,
    address_or_offset: u32,
    element_count: u32,
    has_function: bool,
    parent: Option<u32>,
}

/// Host-side symbol storage: metadata, generic primitive values and typed class
/// instances, indexed by symbol id.
#[derive(Debug, Default, Clone)]
pub struct TypedRegistry {
    symbols: Vec<SymbolEntry>,
    instances: Vec<Option<TypedInstance>>,
    /// Runtime copies of instance symbols, used for duplicated spawns of the
    /// same instance (each spawned object gets its own record). Keyed by ids
    /// outside the symbol id range.
    copies: HashMap<u32, TypedInstance>,
    /// Source symbol of every runtime copy id.
    copy_sources: HashMap<u32, u32>,
    /// Value slot word 0 of instance symbols. For classes whose members start
    /// past offset 0 it holds the instance reference (for example `self`/`other`
    /// values); for other classes it is unused.
    references: Vec<i32>,
    /// Initial values of symbols with a value slot. `None` means all-zero
    /// defaults. Used to skip unchanged symbols when saving and to restore the
    /// default state of symbols which were intentionally not saved.
    defaults: Vec<Option<Vec<i32>>>,
    classes: [Option<u32>; 5],
    finished: bool,
}

impl TypedRegistry {
    /// Creates an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds the next symbol. Symbols must arrive in ascending symbol-id order.
    ///
    /// A class without a typed model is reported and its instances keep generic
    /// storage.
    pub fn add_symbol(&mut self, decl: SymbolDecl<'_>) -> u32 {
        assert!(!self.finished, "add_symbol called after finish");
        let symbol = u32::try_from(self.symbols.len()).unwrap_or(u32::MAX);
        assert!(
            decl.parent.is_none_or(|parent| parent < symbol),
            "parent of symbol({symbol}) is not an earlier symbol"
        );
        if decl.kind == SymbolKind::Class && ClassKind::from_name(decl.name).is_none() {
            println!(
                "error: daedalus class '{}' has no typed model; instances use GenericVariable",
                decl.name
            );
        }

        let storage = if decl.is_class_member {
            None
        } else {
            GenericVariable::new(decl.kind, decl.element_count).map(TypedInstance::Generic)
        };

        self.symbols.push(SymbolEntry {
            name: decl.name.to_owned(),
            kind: decl.kind,
            is_const: decl.is_const,
            is_class_member: decl.is_class_member,
            address_or_offset: decl.address_or_offset,
            element_count: decl.element_count,
            has_function: decl.has_function,
            parent: decl.parent,
        });
        self.instances.push(storage);
        self.references.push(0);
        self.defaults.push(None);
        symbol
    }

    /// Builds name indices, validates modeled classes and allocates typed
    /// instance storage. Panics when a modeled class member does not match the
    /// hardcoded layout.
    pub fn finish(&mut self) {
        assert!(!self.finished, "finish called twice");
        for class in CLASS_KINDS {
            self.validate_class(class);
        }

        for symbol in 0..self.symbols.len() {
            let symbol = u32::try_from(symbol).unwrap_or(u32::MAX);
            let kind = self.symbols[symbol as usize].kind;
            if !matches!(kind, SymbolKind::Instance | SymbolKind::Prototype) {
                continue;
            }
            let Some(class_symbol) = self.class_symbol_of(symbol) else {
                continue;
            };
            let entry = &self.symbols[symbol as usize];
            if kind == SymbolKind::Instance && is_instance_variable(entry) {
                continue;
            }
            let class = ClassKind::from_name(&self.symbols[class_symbol as usize].name);
            let typed =
                class.is_some_and(|class| self.classes[class.index()] == Some(class_symbol));
            if typed && kind == SymbolKind::Instance {
                let class = class.expect("validated class is present");
                self.instances[symbol as usize] = Some(typed_instance(class));
                continue;
            }

            let element_count = self.symbols[symbol as usize].element_count;
            let words = self.record_words(class_symbol, element_count);
            if let Some(TypedInstance::Generic(variable)) = self.instances[symbol as usize].as_mut()
            {
                variable.resize(words);
            }
        }

        self.finished = true;
    }

    fn validate_class(&mut self, class: ClassKind) {
        let Some(class_symbol) = self.class_symbol(class.name()) else {
            return;
        };
        for slot in class.field_slots() {
            let member = self
                .find_member_id(class.name(), slot.member)
                .unwrap_or_else(|| {
                    panic!(
                        "G2 script layout mismatch: {}.{} is missing",
                        class.name(),
                        slot.member
                    )
                });
            let entry = &self.symbols[member as usize];
            assert_eq!(
                entry.address_or_offset,
                slot.offset,
                "G2 script layout mismatch: {}.{} offset is {} instead of {}",
                class.name(),
                slot.member,
                entry.address_or_offset,
                slot.offset
            );
            assert!(
                entry.element_count >= slot.element + slot.count,
                "G2 script layout mismatch: {}.{} has {} elements, {} needed",
                class.name(),
                slot.member,
                entry.element_count,
                slot.element + slot.count
            );
        }
        self.classes[class.index()] = Some(class_symbol);
    }

    fn record_words(&self, class_symbol: u32, element_count: u32) -> usize {
        let mut words = usize::try_from(element_count).unwrap_or_default();
        for entry in &self.symbols {
            if !entry.is_class_member || entry.parent != Some(class_symbol) {
                continue;
            }
            let offset = usize::try_from(entry.address_or_offset / 4).unwrap_or_default();
            let count = usize::try_from(entry.element_count).unwrap_or_default();
            words = words.max(offset + count);
        }
        words
    }

    fn member_covers(&self, class_symbol: u32, byte_offset: u32) -> bool {
        self.symbols.iter().any(|entry| {
            entry.is_class_member
                && entry.parent == Some(class_symbol)
                && entry.address_or_offset <= byte_offset
                && byte_offset < entry.address_or_offset + entry.element_count * 4
        })
    }

    /// Number of instances with a typed class model.
    #[must_use]
    pub fn typed_count(&self) -> usize {
        self.instances
            .iter()
            .flatten()
            .filter(|instance| instance.class().is_some())
            .count()
    }

    /// Value storage of a symbol, `None` when it has no value slot.
    #[must_use]
    pub fn instance(&self, instance: u32) -> Option<&TypedInstance> {
        self.instances
            .get(usize::try_from(instance).ok()?)
            .and_then(Option::as_ref)
            .or_else(|| self.copies.get(&instance))
    }

    /// Mutable value storage of a symbol or runtime copy.
    pub fn instance_mut(&mut self, instance: u32) -> Option<&mut TypedInstance> {
        if self
            .instances
            .get(usize::try_from(instance).ok()?)
            .is_some_and(Option::is_some)
        {
            return self
                .instances
                .get_mut(usize::try_from(instance).ok()?)
                .and_then(Option::as_mut);
        }
        self.copies.get_mut(&instance)
    }

    /// Creates an independent runtime copy of an instance record under a new id.
    /// Returns `false` when the source has no typed instance storage.
    pub fn copy_instance(&mut self, source: u32, instance: u32) -> bool {
        if self.copies.contains_key(&instance) {
            return true;
        }
        let Some(copy) = self.instance(source).and_then(TypedInstance::deep_copy) else {
            return false;
        };
        self.copies.insert(instance, copy);
        self.copy_sources.insert(instance, source);
        true
    }

    /// Whether the id is a runtime copy created by [`Self::copy_instance`].
    #[must_use]
    pub fn is_copy(&self, instance: u32) -> bool {
        self.copies.contains_key(&instance)
    }

    /// Source symbol of a runtime copy id.
    #[must_use]
    pub fn copy_source(&self, instance: u32) -> Option<u32> {
        self.copy_sources.get(&instance).copied()
    }

    /// Ids of every runtime copy.
    pub fn copy_ids(&self) -> impl Iterator<Item = u32> + '_ {
        self.copies.keys().copied()
    }

    /// Drops every runtime copy, the symbol records stay untouched.
    pub fn clear_copies(&mut self) {
        self.copies.clear();
        self.copy_sources.clear();
    }

    /// Typed class of an instance, `None` without a typed model.
    #[must_use]
    pub fn class_of(&self, instance: u32) -> Option<ClassKind> {
        self.instance(instance)?.class()
    }

    /// Whether the class was validated and has typed instances in this registry.
    #[must_use]
    pub fn is_typed_class(&self, class: ClassKind) -> bool {
        self.classes[class.index()].is_some()
    }

    /// Number of declared symbols.
    #[must_use]
    pub fn symbol_count(&self) -> usize {
        self.symbols.len()
    }

    /// Name of a symbol, `None` when the id is out of range.
    #[must_use]
    pub fn name(&self, symbol: u32) -> Option<&str> {
        self.symbols
            .get(usize::try_from(symbol).ok()?)
            .map(|entry| entry.name.as_str())
    }

    /// Kind of a symbol, `None` when the id is out of range.
    #[must_use]
    pub fn kind(&self, symbol: u32) -> Option<SymbolKind> {
        self.symbols
            .get(usize::try_from(symbol).ok()?)
            .map(|entry| entry.kind)
    }

    /// Whether a symbol has wasm function code.
    #[must_use]
    pub fn has_function(&self, symbol: u32) -> bool {
        self.symbols
            .get(usize::try_from(symbol).unwrap_or(usize::MAX))
            .is_some_and(|entry| entry.has_function)
    }

    /// Whether a symbol is declared `const`; such symbols are readable but
    /// writes are ignored.
    #[must_use]
    pub fn is_const(&self, symbol: u32) -> bool {
        self.symbols
            .get(usize::try_from(symbol).unwrap_or(usize::MAX))
            .is_some_and(|entry| entry.is_const)
    }

    /// Whether a symbol is a class member living inside its parent instance
    /// record instead of having its own value slot.
    #[must_use]
    pub fn is_class_member(&self, symbol: u32) -> bool {
        self.symbols
            .get(usize::try_from(symbol).unwrap_or(usize::MAX))
            .is_some_and(|entry| entry.is_class_member)
    }

    /// Records the initial values of a symbol with a value slot. All-zero
    /// values are stored as "no non-zero default".
    pub fn set_default_values(&mut self, symbol: u32, values: &[i32]) {
        let Ok(index) = usize::try_from(symbol) else {
            return;
        };
        if self.defaults.len() <= index {
            self.defaults.resize(index + 1, None);
        }
        self.defaults[index] = if values.iter().all(|value| *value == 0) {
            None
        } else {
            Some(values.to_vec())
        };
    }

    /// Whether a value vector equals the symbol's default values. Extra words
    /// past the stored default must be zero (default storage is sparse).
    #[must_use]
    pub fn matches_default(&self, symbol: u32, values: &[i32]) -> bool {
        let Some(default) = self
            .defaults
            .get(usize::try_from(symbol).unwrap_or(usize::MAX))
            .and_then(Option::as_ref)
        else {
            return values.iter().all(|value| *value == 0);
        };
        values.len() >= default.len()
            && values
                .iter()
                .zip(default)
                .all(|(value, default)| value == default)
            && values[default.len()..].iter().all(|value| *value == 0)
    }

    /// Restores a symbol's value slot to its recorded defaults, zeroing every
    /// modeled word first so init-time writes do not survive.
    pub fn reset_to_default(&mut self, symbol: u32) {
        if !self.has_value_slot(symbol) || self.is_class_member(symbol) {
            return;
        }
        let Ok(index) = usize::try_from(symbol) else {
            return;
        };
        let default = self.defaults.get(index).and_then(Option::as_ref).cloned();
        let Some(storage) = self.instances.get(index).and_then(Option::as_ref) else {
            return;
        };
        let words = if let TypedInstance::Generic(variable) = storage {
            variable.len().max(default.as_ref().map_or(0, Vec::len))
        } else if let Some(class) = storage.class() {
            typed_words_len(class)
        } else {
            return;
        };

        let mut values = vec![0; words];
        if let Some(default) = default {
            for (value, default) in values.iter_mut().zip(default) {
                *value = default;
            }
        }
        self.restore_values(symbol, &values);
    }

    /// Word indices of a symbol's string values, for dynamic-string sanitizing.
    /// A string global returns its own slot words; an instance returns the word
    /// offsets of the string members of its class. Other kinds return an empty
    /// list.
    #[must_use]
    pub fn string_word_indices(&self, symbol: u32) -> Vec<u32> {
        let Ok(symbol_index) = usize::try_from(symbol) else {
            return Vec::new();
        };
        let Some(entry) = self.symbols.get(symbol_index) else {
            return Vec::new();
        };
        if entry.kind == SymbolKind::String {
            return (0..entry.element_count).collect();
        }
        if entry.kind != SymbolKind::Instance {
            return Vec::new();
        }
        let Some(class_symbol) = self.class_symbol_of(symbol) else {
            return Vec::new();
        };

        let mut indices = Vec::new();
        for member in &self.symbols {
            if !member.is_class_member
                || member.parent != Some(class_symbol)
                || member.kind != SymbolKind::String
            {
                continue;
            }
            let start = member.address_or_offset / 4;
            for element in 0..member.element_count {
                if let Some(index) = start.checked_add(element) {
                    indices.push(index);
                }
            }
        }
        indices
    }

    /// Whether a symbol has its own value slot.
    #[must_use]
    pub fn has_value_slot(&self, symbol: u32) -> bool {
        self.instances
            .get(usize::try_from(symbol).unwrap_or(usize::MAX))
            .is_some_and(Option::is_some)
    }

    /// Symbols of kind [`SymbolKind::Instance`], in symbol order.
    pub fn instance_symbols(&self) -> impl Iterator<Item = u32> + '_ {
        self.symbols
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| {
                (entry.kind == SymbolKind::Instance)
                    .then(|| u32::try_from(index).ok())
                    .flatten()
            })
    }

    /// Generic storage of a symbol, `None` for typed classes and symbols
    /// without a value slot.
    #[must_use]
    pub fn variable(&self, symbol: u32) -> Option<&GenericVariable> {
        self.instance(symbol)?.generic()
    }

    /// Mutable generic storage of a symbol.
    pub fn variable_mut(&mut self, symbol: u32) -> Option<&mut GenericVariable> {
        self.instances
            .get_mut(usize::try_from(symbol).ok()?)?
            .as_mut()?
            .generic_mut()
    }

    /// Reads element `element` of a global/variable symbol.
    #[must_use]
    pub fn read_symbol(&self, symbol: u32, element: u32) -> i32 {
        let Some(instance) = self.instance(symbol) else {
            println!("error: read of symbol({symbol}) without a value slot");
            return -1;
        };
        if let TypedInstance::Generic(variable) = instance {
            return variable.get(element).unwrap_or_else(|| {
                println!(
                    "error: read of symbol({symbol}), element({element}) outside {} words",
                    variable.len()
                );
                -1
            });
        }
        let Some(class) = instance.class() else {
            return -1;
        };
        if element == 0 && class.uses_reference_word() {
            if self.is_copy(symbol) {
                return i32::try_from(symbol).unwrap_or_default();
            }
            return self
                .references
                .get(usize::try_from(symbol).unwrap_or(usize::MAX))
                .copied()
                .unwrap_or_default();
        }
        let Some(byte_offset) = element.checked_mul(4) else {
            println!("error: read of symbol({symbol}), element({element}) overflows");
            return -1;
        };
        self.read_typed_word(symbol, class, instance, byte_offset)
    }

    /// Writes element `element` of a global/variable symbol. Writes to const
    /// symbols are rejected, their values only come from the initial defaults.
    pub fn write_symbol(&mut self, symbol: u32, element: u32, value: i32) {
        if self.is_const(symbol) {
            println!("error: write of const symbol({symbol}), element({element}) ignored");
            return;
        }
        self.write_symbol_unchecked(symbol, element, value);
    }

    /// Writes a symbol value slot without the const check, used for the initial
    /// defaults of const globals.
    fn write_symbol_unchecked(&mut self, symbol: u32, element: u32, value: i32) {
        let symbol_index = usize::try_from(symbol).unwrap_or(usize::MAX);
        let reference_class = self.instance(symbol).and_then(TypedInstance::class);
        if element == 0 && reference_class.is_some_and(ClassKind::uses_reference_word) {
            if self.is_copy(symbol) {
                // A copy id is its own reference, the source word stays untouched.
                return;
            }
            if let Some(reference) = self.references.get_mut(symbol_index) {
                *reference = value;
                return;
            }
        }
        let Some(instance) = self.instance_mut(symbol) else {
            println!("error: write of symbol({symbol}) without a value slot");
            return;
        };
        if let TypedInstance::Generic(variable) = instance {
            if !variable.set(element, value) {
                println!("error: write of symbol({symbol}), element({element}) failed");
            }
            return;
        }
        let Some(class) = instance.class() else {
            return;
        };
        let Some(byte_offset) = element.checked_mul(4) else {
            println!("error: write of symbol({symbol}), element({element}) overflows");
            return;
        };
        let Some((slot, element)) = flat_word_slot(class, byte_offset) else {
            println!(
                "error: {}.<instance> offset({byte_offset}) is not modeled (write)",
                class.name()
            );
            return;
        };
        if !instance.write(slot.offset, element, value) {
            println!(
                "error: {}.{}[{}] is not modeled (write)",
                class.name(),
                slot.member,
                element
            );
        }
    }

    /// Snapshot of every word of a symbol's value slot. Generic symbols return
    /// their stored words; typed instances return a dense word vector with the
    /// reference word first (when the class uses it) and every modeled field at
    /// its record offset. Gaps of unmodeled record space stay zero.
    #[must_use]
    pub fn snapshot_values(&self, symbol: u32) -> Option<Vec<i32>> {
        let storage = self.instance(symbol)?;
        if let TypedInstance::Generic(variable) = storage {
            return Some(variable.words().to_vec());
        }

        let class = storage.class()?;
        let mut values = vec![0; typed_words_len(class)];
        if class.uses_reference_word() {
            let reference = self
                .references
                .get(usize::try_from(symbol).ok()?)
                .copied()
                .unwrap_or_default();
            if let Some(first) = values.first_mut() {
                *first = reference;
            }
        }
        for (index, value) in values.iter_mut().enumerate() {
            let byte_offset = u32::try_from(index).ok()?.checked_mul(4)?;
            if byte_offset == 0 && class.uses_reference_word() {
                continue;
            }
            let Some((slot, element)) = flat_word_slot(class, byte_offset) else {
                continue;
            };
            if let Some(stored) = storage.read(slot.offset, element) {
                *value = stored;
            }
        }
        Some(values)
    }

    /// Writes a dense word vector produced by [`Self::snapshot_values`] back
    /// into a symbol's value slot. Gaps of unmodeled record space and words past
    /// the modeled end are ignored.
    pub fn restore_values(&mut self, symbol: u32, values: &[i32]) {
        let symbol_index = usize::try_from(symbol).unwrap_or(usize::MAX);
        let Some(storage) = self
            .instances
            .get_mut(symbol_index)
            .and_then(Option::as_mut)
        else {
            println!("error: restore of symbol({symbol}) without a value slot");
            return;
        };
        if let TypedInstance::Generic(variable) = storage {
            for (index, value) in values.iter().enumerate() {
                if let Ok(index) = u32::try_from(index) {
                    variable.set(index, *value);
                }
            }
            return;
        }

        let Some(class) = storage.class() else {
            return;
        };
        let uses_reference = class.uses_reference_word();
        for (index, value) in values.iter().enumerate() {
            if index == 0 && uses_reference {
                if let Some(reference) = self.references.get_mut(symbol_index) {
                    *reference = *value;
                }
                continue;
            }
            let Ok(index) = u32::try_from(index) else {
                continue;
            };
            let Some(byte_offset) = index.checked_mul(4) else {
                continue;
            };
            let Some((slot, element)) = flat_word_slot(class, byte_offset) else {
                continue;
            };
            storage.write(slot.offset, element, *value);
        }
    }

    /// Snapshot of a runtime copy. The reference word holds the copy id, the
    /// remaining words follow [`Self::snapshot_values`].
    #[must_use]
    pub fn snapshot_copy(&self, instance: u32) -> Option<Vec<i32>> {
        let storage = self.copies.get(&instance)?;
        if let TypedInstance::Generic(variable) = storage {
            return Some(variable.words().to_vec());
        }
        let class = storage.class()?;
        let mut values = vec![0; typed_words_len(class)];
        if class.uses_reference_word()
            && let Some(first) = values.first_mut()
        {
            *first = i32::try_from(instance).unwrap_or_default();
        }
        for (index, value) in values.iter_mut().enumerate() {
            let byte_offset = u32::try_from(index).ok()?.checked_mul(4)?;
            if byte_offset == 0 && class.uses_reference_word() {
                continue;
            }
            let Some((slot, element)) = flat_word_slot(class, byte_offset) else {
                continue;
            };
            if let Some(stored) = storage.read(slot.offset, element) {
                *value = stored;
            }
        }
        Some(values)
    }

    /// Restores a word vector produced by [`Self::snapshot_copy`]. The reference
    /// word is the copy id and is not written into the record.
    pub fn restore_copy(&mut self, instance: u32, values: &[i32]) {
        let Some(storage) = self.copies.get_mut(&instance) else {
            println!("error: restore of missing copy({instance})");
            return;
        };
        if let TypedInstance::Generic(variable) = storage {
            for (index, value) in values.iter().enumerate() {
                if let Ok(index) = u32::try_from(index) {
                    variable.set(index, *value);
                }
            }
            return;
        }
        let Some(class) = storage.class() else {
            return;
        };
        let uses_reference = class.uses_reference_word();
        for (index, value) in values.iter().enumerate() {
            if index == 0 && uses_reference {
                continue;
            }
            let Ok(index) = u32::try_from(index) else {
                continue;
            };
            let Some(byte_offset) = index.checked_mul(4) else {
                continue;
            };
            let Some((slot, element)) = flat_word_slot(class, byte_offset) else {
                continue;
            };
            storage.write(slot.offset, element, *value);
        }
    }

    /// Writes every word of a symbol's initial values.
    pub fn write_values(&mut self, symbol: u32, values: &[i32]) {
        let class = self.instance(symbol).and_then(TypedInstance::class);
        let class_symbol = self.class_symbol_of(symbol);
        for (element, value) in values.iter().enumerate() {
            let element = u32::try_from(element).unwrap_or(u32::MAX);
            if let (Some(class), Some(class_symbol)) = (class, class_symbol) {
                let byte_offset = element.saturating_mul(4);
                let is_reference = byte_offset == 0 && class.uses_reference_word();
                if !is_reference
                    && flat_word_slot(class, byte_offset).is_none()
                    && !self.member_covers(class_symbol, byte_offset)
                {
                    continue;
                }
            }
            self.write_symbol_unchecked(symbol, element, *value);
        }
    }

    /// Reads the instance record word at `byte_offset`.
    #[must_use]
    pub fn read_instance_word(&self, instance: u32, byte_offset: u32) -> i32 {
        let Some(storage) = self.instance(instance) else {
            println!("error: read of instance({instance}) without a value slot");
            return -1;
        };
        if let TypedInstance::Generic(variable) = storage {
            return variable.get(byte_offset / 4).unwrap_or_else(|| {
                println!(
                    "error: read of instance({instance}), offset({byte_offset}) outside {} words",
                    variable.len()
                );
                -1
            });
        }
        let Some(class) = storage.class() else {
            return -1;
        };
        if byte_offset == 0 && class.uses_reference_word() {
            return self
                .references
                .get(usize::try_from(instance).unwrap_or(usize::MAX))
                .copied()
                .unwrap_or_default();
        }
        self.read_typed_word(instance, class, storage, byte_offset)
    }

    /// Writes the instance record word at `byte_offset`.
    pub fn write_instance_word(&mut self, instance: u32, byte_offset: u32, value: i32) {
        let instance_index = usize::try_from(instance).unwrap_or(usize::MAX);
        let reference_class = self
            .instances
            .get(instance_index)
            .and_then(Option::as_ref)
            .and_then(TypedInstance::class);
        if byte_offset == 0
            && reference_class.is_some_and(ClassKind::uses_reference_word)
            && let Some(reference) = self.references.get_mut(instance_index)
        {
            *reference = value;
            return;
        }
        let Some(storage) = self
            .instances
            .get_mut(instance_index)
            .and_then(Option::as_mut)
        else {
            println!("error: write of instance({instance}) without a value slot");
            return;
        };
        if let TypedInstance::Generic(variable) = storage {
            if !variable.set(byte_offset / 4, value) {
                println!("error: write of instance({instance}), offset({byte_offset}) failed");
            }
            return;
        }
        let Some(class) = storage.class() else {
            return;
        };
        let Some((slot, element)) = flat_word_slot(class, byte_offset) else {
            println!(
                "error: {}.<instance> offset({byte_offset}) is not modeled (write)",
                class.name()
            );
            return;
        };
        if !storage.write(slot.offset, element, value) {
            println!(
                "error: {}.{}[{}] is not modeled (write)",
                class.name(),
                slot.member,
                element
            );
        }
    }

    /// Reads a class member of an instance.
    #[must_use]
    pub fn read_member(&self, instance: u32, member: u32, element: u32) -> i32 {
        let Some(storage) = self.instance(instance) else {
            println!("error: read of instance({instance}) without a value slot");
            return -1;
        };
        let Some(entry) = self.member(instance, member) else {
            return -1;
        };
        if let TypedInstance::Generic(variable) = storage {
            let index = entry.address_or_offset / 4 + element;
            return variable.get(index).unwrap_or_else(|| {
                println!(
                    "error: read of {}.{member}[{element}] outside {} words",
                    entry.name,
                    variable.len()
                );
                -1
            });
        }
        let Some(class) = storage.class() else {
            return -1;
        };
        storage
            .read(entry.address_or_offset, element)
            .unwrap_or_else(|| {
                println!(
                    "error: {}.{}[{}] is not modeled (read)",
                    class.name(),
                    member_name(entry),
                    element
                );
                -1
            })
    }

    /// Writes a class member of an instance.
    pub fn write_member(&mut self, instance: u32, member: u32, element: u32, value: i32) {
        let Some(entry) = self.member(instance, member) else {
            return;
        };
        let offset = entry.address_or_offset;
        let name = member_name(entry).to_owned();
        let Some(storage) = self.instance_mut(instance) else {
            println!("error: write of instance({instance}) without a value slot");
            return;
        };
        if let TypedInstance::Generic(variable) = storage {
            if !variable.set(offset / 4 + element, value) {
                println!("error: write of instance({instance}), member({member}) failed");
            }
            return;
        }
        let Some(class) = storage.class() else {
            return;
        };
        if !storage.write(offset, element, value) {
            println!(
                "error: {}.{}[{}] is not modeled (write)",
                class.name(),
                name,
                element
            );
        }
    }

    /// Snapshot of an npc instance.
    #[must_use]
    pub fn npc(&self, instance: u32) -> Option<Npc> {
        match self.instance(instance)? {
            TypedInstance::Npc(handle) => Some(read_lock(handle).clone()),
            _ => None,
        }
    }

    /// Sets `C_Npc.wp`, the waypoint the npc is currently assigned to. The
    /// engine writes it when a routine or AI state starts and scripts read it
    /// back through `self.wp` (for example `C_RefuseTalk`). `false` when the
    /// instance is not an npc.
    pub fn set_npc_waypoint(&mut self, instance: u32, waypoint: Option<StringId>) -> bool {
        let Some(storage) = self.instance_mut(instance) else {
            return false;
        };
        let TypedInstance::Npc(handle) = storage else {
            return false;
        };
        write_lock(handle).waypoint = waypoint;
        true
    }

    /// Snapshot of an item instance.
    #[must_use]
    pub fn item(&self, instance: u32) -> Option<Item> {
        match self.instance(instance)? {
            TypedInstance::Item(handle) => Some(read_lock(handle).clone()),
            _ => None,
        }
    }

    /// Snapshot of a menu instance.
    #[must_use]
    pub fn menu(&self, instance: u32) -> Option<Menu> {
        match self.instance(instance)? {
            TypedInstance::Menu(handle) => Some(read_lock(handle).clone()),
            _ => None,
        }
    }

    /// Snapshot of a menu item instance.
    #[must_use]
    pub fn menu_item(&self, instance: u32) -> Option<MenuItem> {
        match self.instance(instance)? {
            TypedInstance::MenuItem(handle) => Some(read_lock(handle).clone()),
            _ => None,
        }
    }

    /// Snapshot of a dialog info instance.
    #[must_use]
    pub fn info(&self, instance: u32) -> Option<Info> {
        match self.instance(instance)? {
            TypedInstance::Info(handle) => Some(read_lock(handle).clone()),
            _ => None,
        }
    }

    fn read_typed_word(
        &self,
        instance: u32,
        class: ClassKind,
        storage: &TypedInstance,
        byte_offset: u32,
    ) -> i32 {
        let Some((slot, element)) = flat_word_slot(class, byte_offset) else {
            println!(
                "error: {}.<instance> offset({byte_offset}) is not modeled (read)",
                class.name()
            );
            return -1;
        };
        storage.read(slot.offset, element).unwrap_or_else(|| {
            println!(
                "error: read of instance({instance}), {}.{}[{}] is not modeled",
                class.name(),
                slot.member,
                element
            );
            -1
        })
    }

    fn member(&self, instance: u32, member: u32) -> Option<&SymbolEntry> {
        let entry = self.symbols.get(usize::try_from(member).ok()?)?;
        if !entry.is_class_member {
            println!("error: symbol({member}) is not a class member");
            return None;
        }
        if let Some(parent) = entry.parent
            && let Some(instance_class) = self.class_symbol_of(instance)
            && instance_class != parent
        {
            println!("error: member({member}) does not belong to instance({instance})");
            return None;
        }
        Some(entry)
    }

    fn class_symbol(&self, class_name: &str) -> Option<u32> {
        self.symbols.iter().enumerate().find_map(|(index, entry)| {
            (entry.kind == SymbolKind::Class && entry.name.eq_ignore_ascii_case(class_name))
                .then(|| u32::try_from(index).ok())
                .flatten()
        })
    }

    fn find_member_id(&self, class_name: &str, member_name: &str) -> Option<u32> {
        let class = self.class_symbol(class_name)?;
        self.symbols.iter().enumerate().find_map(|(index, entry)| {
            if !entry.is_class_member || entry.parent != Some(class) {
                return None;
            }
            member_name_str(&entry.name)
                .eq_ignore_ascii_case(member_name)
                .then(|| u32::try_from(index).ok())
                .flatten()
        })
    }

    fn class_symbol_of(&self, symbol: u32) -> Option<u32> {
        // Runtime copies resolve through their source symbol.
        let mut current = self.copy_sources.get(&symbol).copied().unwrap_or(symbol);
        // Prototypes can form a chain, the class is somewhere at its end.
        for _ in 0..16 {
            let entry = self.symbols.get(usize::try_from(current).ok()?)?;
            if entry.kind == SymbolKind::Class {
                return Some(current);
            }
            current = entry.parent?;
        }
        None
    }
}

fn member_name(entry: &SymbolEntry) -> &str {
    member_name_str(&entry.name)
}

fn is_instance_variable(entry: &SymbolEntry) -> bool {
    entry.name.rsplit_once('.').is_some()
        || ["self", "other", "victim", "item", "hero"]
            .iter()
            .any(|name| entry.name.eq_ignore_ascii_case(name))
}

fn member_name_str(name: &str) -> &str {
    name.rsplit_once('.').map_or(name, |(_, member)| member)
}

/// Number of words of a typed instance record, dense through the last modeled
/// field (or just the reference word when no field is modeled).
fn typed_words_len(class: ClassKind) -> usize {
    let mut words = usize::from(class.uses_reference_word());
    for slot in class.field_slots() {
        let end_word = usize::try_from(slot.offset / 4)
            .unwrap_or_default()
            .saturating_add(usize::try_from(slot.element).unwrap_or_default())
            .saturating_add(usize::try_from(slot.count).unwrap_or_default());
        words = words.max(end_word);
    }
    words
}

fn flat_word_slot(class: ClassKind, byte_offset: u32) -> Option<(&'static FieldSlot, u32)> {
    for slot in class.field_slots() {
        let start = slot.offset + slot.element * 4;
        let end = start + slot.count * 4;
        if start <= byte_offset && byte_offset < end {
            return Some((slot, (byte_offset - slot.offset) / 4));
        }
    }
    None
}

fn typed_instance(class: ClassKind) -> TypedInstance {
    match class {
        ClassKind::Npc => TypedInstance::Npc(Arc::new(RwLock::new(Npc::default()))),
        ClassKind::Item => TypedInstance::Item(Arc::new(RwLock::new(Item::default()))),
        ClassKind::Menu => TypedInstance::Menu(Arc::new(RwLock::new(Menu::default()))),
        ClassKind::MenuItem => TypedInstance::MenuItem(Arc::new(RwLock::new(MenuItem::default()))),
        ClassKind::Info => TypedInstance::Info(Arc::new(RwLock::new(Info::default()))),
    }
}

fn read_lock<T>(lock: &RwLock<T>) -> std::sync::RwLockReadGuard<'_, T> {
    lock.read().unwrap_or_else(PoisonError::into_inner)
}

fn write_lock<T>(lock: &RwLock<T>) -> std::sync::RwLockWriteGuard<'_, T> {
    lock.write().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::StringId;

    fn decl<'a>(
        name: &'a str,
        kind: SymbolKind,
        is_class_member: bool,
        offset: u32,
        elements: u32,
        parent: Option<u32>,
    ) -> SymbolDecl<'a> {
        SymbolDecl {
            name,
            kind,
            is_const: false,
            is_class_member,
            address_or_offset: offset,
            element_count: elements,
            has_function: false,
            parent,
        }
    }

    fn add_npc_symbols(registry: &mut TypedRegistry) -> (u32, u32) {
        let class = registry.add_symbol(decl("c_npc", SymbolKind::Class, false, 0, 0, None));
        for (name, offset, elements) in [
            ("id", 288, 1),
            ("name", 292, 5),
            ("slot", 392, 1),
            ("effect", 412, 1),
            ("npctype", 432, 1),
            ("flags", 436, 1),
            ("attribute", 440, 8),
            ("hitchance", 472, 5),
            ("protection", 492, 8),
            ("damage", 524, 8),
            ("damagetype", 556, 1),
            ("guild", 560, 1),
            ("level", 564, 1),
            ("mission", 568, 5),
            ("fight_tactic", 588, 1),
            ("weapon", 592, 1),
            ("voice", 596, 1),
            ("voicepitch", 600, 1),
            ("bodymass", 604, 1),
            ("daily_routine", 608, 1),
            ("start_aistate", 612, 1),
            ("spawnpoint", 616, 1),
            ("spawndelay", 636, 1),
            ("senses", 640, 1),
            ("senses_range", 644, 1),
            ("aivar", 648, 100),
            ("wp", 1048, 1),
            ("exp", 1068, 1),
            ("exp_next", 1072, 1),
            ("lp", 1076, 1),
            ("bodystateinterruptableoverride", 1080, 1),
            ("nofocus", 1084, 1),
        ] {
            registry.add_symbol(decl(
                name,
                SymbolKind::Int,
                true,
                offset,
                elements,
                Some(class),
            ));
        }
        let instance = registry.add_symbol(decl(
            "test_npc",
            SymbolKind::Instance,
            false,
            0,
            272,
            Some(class),
        ));
        (class, instance)
    }

    fn npc_registry() -> (TypedRegistry, u32) {
        let mut registry = TypedRegistry::new();
        let (_, instance) = add_npc_symbols(&mut registry);
        registry.finish();

        let mut defaults = vec![0; 272];
        defaults[288 / 4] = 5;
        defaults[292 / 4] = 7;
        defaults[296 / 4] = 9;
        defaults[392 / 4] = 100;
        defaults[412 / 4] = 101;
        defaults[432 / 4] = 2;
        defaults[436 / 4] = 4;
        defaults[440 / 4] = 100;
        defaults[440 / 4 + 1] = 200;
        defaults[440 / 4 + 2] = 55;
        defaults[440 / 4 + 6] = 77;
        defaults[472 / 4] = 11;
        defaults[492 / 4 + 1] = 12;
        defaults[524 / 4 + 2] = 13;
        defaults[556 / 4] = 3;
        defaults[560 / 4] = 6;
        defaults[564 / 4] = 7;
        defaults[568 / 4] = 8;
        defaults[588 / 4] = 9;
        defaults[592 / 4] = 10;
        defaults[596 / 4] = 11;
        defaults[600 / 4] = 12;
        defaults[604 / 4] = 80;
        defaults[612 / 4] = 300;
        defaults[616 / 4] = 102;
        defaults[636 / 4] = 5;
        defaults[640 / 4] = 6;
        defaults[644 / 4] = 1000;
        defaults[648 / 4 + 4] = 3;
        defaults[1068 / 4] = 1000;
        defaults[1080 / 4] = 1;
        defaults[1084 / 4] = 1;
        registry.write_values(instance, &defaults);
        (registry, instance)
    }

    #[test]
    fn runtime_copy_is_an_independent_npc_record() {
        let (mut registry, instance) = npc_registry();
        let copy = 1_000_000u32;
        assert!(registry.copy_instance(instance, copy));
        assert!(registry.is_copy(copy));
        assert_eq!(registry.copy_source(copy), Some(instance));

        let attribute = registry
            .find_member_id("c_npc", "attribute")
            .expect("member");
        let original_hitpoints = registry.read_member(instance, attribute, 0);
        registry.write_member(copy, attribute, 0, 5);
        assert_eq!(registry.read_member(copy, attribute, 0), 5);
        assert_eq!(
            registry.read_member(instance, attribute, 0),
            original_hitpoints
        );
        assert_eq!(registry.npc(copy).expect("copy npc").guild, 6);
        assert_eq!(registry.read_symbol(copy, 0), copy as i32);

        let values = registry.snapshot_copy(copy).expect("copy snapshot");
        registry.write_member(copy, attribute, 0, 9);
        registry.restore_copy(copy, &values);
        assert_eq!(registry.read_member(copy, attribute, 0), 5);

        registry.clear_copies();
        assert!(!registry.is_copy(copy));
        assert!(registry.npc(copy).is_none());
    }

    #[test]
    fn typed_npc_reads_and_writes_semantic_fields() {
        let (mut registry, instance) = npc_registry();

        assert_eq!(registry.typed_count(), 1);
        let npc = registry.npc(instance).expect("typed npc");
        assert_eq!(npc.name, StringId::new(7));
        assert_eq!(npc.name_extra[0], StringId::new(9));
        assert_eq!(npc.hitpoints, 100);
        assert_eq!(npc.hitpoints_max, 200);
        assert_eq!(npc.mana, 55);
        assert_eq!(npc.exp, 1000);
        assert_eq!(npc.aivar[4], 3);
        assert_eq!(npc.daily_routine, None);
        assert_eq!(npc.id, 5);
        assert_eq!(npc.slot, StringId::new(100));
        assert_eq!(npc.effect, StringId::new(101));
        assert_eq!(npc.npctype, 2);
        assert_eq!(npc.flags, 4);
        assert_eq!(npc.hitchance[0], 11);
        assert_eq!(npc.protection[1], 12);
        assert_eq!(npc.damage[2], 13);
        assert_eq!(npc.regenerate_hp, 77);
        assert_eq!(npc.damagetype, 3);
        assert_eq!(npc.guild, 6);
        assert_eq!(npc.level, 7);
        assert_eq!(npc.mission[0], crate::ids::FunctionId::new(8));
        assert_eq!(npc.fight_tactic, 9);
        assert_eq!(npc.weapon, 10);
        assert_eq!(npc.voice, 11);
        assert_eq!(npc.voicepitch, 12);
        assert_eq!(npc.bodymass, 80);
        assert_eq!(npc.start_aistate, crate::ids::FunctionId::new(300));
        assert_eq!(npc.spawnpoint, StringId::new(102));
        assert_eq!(npc.spawndelay, 5);
        assert_eq!(npc.senses, 6);
        assert_eq!(npc.senses_range, 1000);
        assert_eq!(npc.bodystateinterruptableoverride, 1);
        assert_eq!(npc.nofocus, 1);

        assert_eq!(registry.read_instance_word(instance, 292), 7);
        assert_eq!(registry.read_instance_word(instance, 296), 9);
        assert_eq!(registry.read_instance_word(instance, 440), 100);
        assert_eq!(registry.read_instance_word(instance, 440 + 6 * 4), 77);
        assert_eq!(registry.read_symbol(instance, 0), 0);
        registry.write_symbol(instance, 0, 99);
        assert_eq!(registry.read_symbol(instance, 0), 99);
        assert_eq!(registry.read_instance_word(instance, 0), 99);

        let attribute = registry
            .find_member_id("c_npc", "attribute")
            .expect("member");
        assert_eq!(registry.read_member(instance, attribute, 0), 100);
        assert_eq!(registry.read_member(instance, attribute, 6), 77);

        registry.write_member(instance, attribute, 0, 50);
        assert_eq!(registry.npc(instance).expect("npc").hitpoints, 50);
    }

    #[test]
    #[should_panic(expected = "G2 script layout mismatch")]
    fn offset_mismatch_panics() {
        let mut registry = TypedRegistry::new();
        let class = registry.add_symbol(decl("c_npc", SymbolKind::Class, false, 0, 0, None));
        for (name, offset, elements) in [
            ("name", 292, 5),
            ("attribute", 444, 8),
            ("daily_routine", 608, 1),
            ("aivar", 648, 100),
            ("wp", 1048, 1),
            ("exp", 1068, 1),
            ("exp_next", 1072, 1),
            ("lp", 1076, 1),
        ] {
            registry.add_symbol(decl(
                name,
                SymbolKind::Int,
                true,
                offset,
                elements,
                Some(class),
            ));
        }
        registry.add_symbol(decl(
            "test_npc",
            SymbolKind::Instance,
            false,
            0,
            270,
            Some(class),
        ));
        registry.finish();
    }

    #[test]
    fn member_of_another_class_is_not_modeled() {
        let mut registry = TypedRegistry::new();
        let (_, instance) = add_npc_symbols(&mut registry);
        let other = registry.add_symbol(decl("c_other", SymbolKind::Class, false, 0, 0, None));
        let member = registry.add_symbol(decl("value", SymbolKind::Int, true, 292, 1, Some(other)));
        registry.finish();

        assert_eq!(registry.read_member(instance, member, 0), -1);
        registry.write_member(instance, member, 0, 5);
        assert_eq!(registry.read_member(instance, member, 0), -1);
    }

    #[test]
    fn primitive_symbols_are_typed_generic_variables() {
        let mut registry = TypedRegistry::new();
        let int = registry.add_symbol(decl("an_int", SymbolKind::Int, false, 0, 1, None));
        let floats = registry.add_symbol(decl("floats", SymbolKind::Float, false, 0, 3, None));
        let string = registry.add_symbol(decl("a_string", SymbolKind::String, false, 0, 1, None));
        registry.finish();

        registry.write_symbol(int, 0, 42);
        assert_eq!(registry.read_symbol(int, 0), 42);
        assert_eq!(registry.variable(int).expect("int").as_int(), Some(42));

        registry.write_symbol(floats, 0, f32::to_bits(1.5) as i32);
        assert_eq!(
            registry
                .variable(floats)
                .expect("floats")
                .as_floats()
                .and_then(|mut values| values.next()),
            Some(1.5)
        );

        registry.write_symbol(string, 0, 7);
        assert_eq!(
            registry.variable(string).expect("string").as_string(),
            StringId::new(7)
        );
    }

    #[test]
    fn instance_without_typed_class_uses_generic_record() {
        let mut registry = TypedRegistry::new();
        let class = registry.add_symbol(decl("c_other", SymbolKind::Class, false, 0, 0, None));
        let member = registry.add_symbol(decl("value", SymbolKind::Int, true, 4, 1, Some(class)));
        let instance = registry.add_symbol(decl(
            "test_other",
            SymbolKind::Instance,
            false,
            0,
            1,
            Some(class),
        ));
        registry.finish();

        assert_eq!(registry.class_of(instance), None);
        assert!(registry.npc(instance).is_none());
        registry.write_member(instance, member, 0, 5);
        assert_eq!(registry.read_member(instance, member, 0), 5);
    }

    #[test]
    fn typed_info_fields() {
        let mut registry = TypedRegistry::new();
        let class = registry.add_symbol(decl("c_info", SymbolKind::Class, false, 0, 0, None));
        for (name, offset) in [
            ("npc", 0),
            ("nr", 4),
            ("important", 8),
            ("condition", 12),
            ("information", 16),
            ("description", 20),
            ("trade", 40),
            ("permanent", 44),
        ] {
            registry.add_symbol(decl(name, SymbolKind::Int, true, offset, 1, Some(class)));
        }
        let instance = registry.add_symbol(decl(
            "test_info",
            SymbolKind::Instance,
            false,
            0,
            12,
            Some(class),
        ));
        registry.finish();

        let mut defaults = vec![0; 12];
        defaults[0] = 12081;
        defaults[1] = 2;
        defaults[2] = 1;
        defaults[3] = 26935;
        defaults[4] = 26936;
        registry.write_values(instance, &defaults);

        assert!(registry.is_typed_class(ClassKind::Info));
        let info = registry.info(instance).expect("typed info");
        assert_eq!(info.npc, crate::ids::InstanceId::new(12081));
        assert_eq!(info.order, 2);
        assert!(info.important);
        assert_eq!(info.condition, crate::ids::FunctionId::new(26935));
        assert_eq!(info.information, crate::ids::FunctionId::new(26936));
        assert_eq!(info.description, None);
        assert!(!info.trade);
        assert!(!info.permanent);

        let nr = registry.find_member_id("c_info", "nr").expect("member");
        registry.write_member(instance, nr, 0, 7);
        assert_eq!(registry.info(instance).expect("info").order, 7);
    }

    #[test]
    fn instance_variables_stay_generic_reference_slots() {
        let mut registry = TypedRegistry::new();
        let (class, instance) = add_npc_symbols(&mut registry);
        let self_var = registry.add_symbol(decl(
            "self",
            SymbolKind::Instance,
            false,
            0,
            200,
            Some(class),
        ));
        let local = registry.add_symbol(decl(
            "caller.self",
            SymbolKind::Instance,
            false,
            0,
            200,
            Some(class),
        ));
        registry.finish();

        assert_eq!(registry.class_of(self_var), None);
        assert_eq!(registry.class_of(local), None);

        registry.write_symbol(self_var, 0, instance as i32);
        assert_eq!(registry.read_symbol(self_var, 0), instance as i32);
    }

    #[test]
    fn const_symbols_have_read_only_storage() {
        let mut registry = TypedRegistry::new();
        let const_array = registry.add_symbol(SymbolDecl {
            name: "txt_talents",
            kind: SymbolKind::String,
            is_const: true,
            is_class_member: false,
            address_or_offset: 0,
            element_count: 3,
            has_function: false,
            parent: None,
        });
        registry.finish();

        assert!(registry.is_const(const_array));
        assert!(registry.has_value_slot(const_array));
        registry.write_values(const_array, &[101, 102, 103]);
        assert_eq!(registry.read_symbol(const_array, 0), 101);
        assert_eq!(registry.read_symbol(const_array, 2), 103);

        registry.write_symbol(const_array, 0, 999);
        assert_eq!(registry.read_symbol(const_array, 0), 101);
    }

    #[test]
    fn class_members_are_reported() {
        let (registry, instance) = npc_registry();
        assert!(!registry.is_class_member(instance));
        let attribute = registry
            .find_member_id("c_npc", "attribute")
            .expect("member");
        assert!(registry.is_class_member(attribute));
    }

    #[test]
    fn string_word_indices_cover_globals_and_class_members() {
        let mut registry = TypedRegistry::new();
        let global = registry.add_symbol(decl("txt_global", SymbolKind::String, false, 0, 3, None));
        let class = registry.add_symbol(decl("c_test", SymbolKind::Class, false, 0, 0, None));
        registry.add_symbol(decl("text", SymbolKind::String, true, 296, 2, Some(class)));
        let instance = registry.add_symbol(decl(
            "test_inst",
            SymbolKind::Instance,
            false,
            0,
            4,
            Some(class),
        ));
        registry.finish();

        assert_eq!(registry.string_word_indices(global), vec![0, 1, 2]);
        assert_eq!(registry.string_word_indices(instance), vec![74, 75]);
    }

    #[test]
    fn snapshot_and_restore_generic_values() {
        let mut registry = TypedRegistry::new();
        let symbol = registry.add_symbol(decl("test_int", SymbolKind::Int, false, 0, 4, None));
        registry.finish();
        registry.write_values(symbol, &[1, 2, 3, 4]);

        let snapshot = registry.snapshot_values(symbol).expect("snapshot");
        assert_eq!(snapshot, vec![1, 2, 3, 4]);

        registry.write_values(symbol, &[9, 9, 9, 9]);
        registry.restore_values(symbol, &snapshot);
        assert_eq!(registry.snapshot_values(symbol), Some(vec![1, 2, 3, 4]));
    }

    #[test]
    fn snapshot_and_restore_typed_instance() {
        let (mut registry, instance) = npc_registry();
        registry.write_symbol(instance, 0, 42);

        let snapshot = registry.snapshot_values(instance).expect("snapshot");
        assert_eq!(snapshot.len(), 272);
        assert_eq!(snapshot[0], 42);
        assert_eq!(snapshot[288 / 4], 5);
        assert_eq!(snapshot[440 / 4 + 1], 200);

        registry.write_symbol(instance, 0, 0);
        registry.write_instance_word(instance, 288, 999);
        registry.write_instance_word(instance, 444, 7);
        assert_ne!(registry.snapshot_values(instance), Some(snapshot.clone()));

        registry.restore_values(instance, &snapshot);
        assert_eq!(registry.snapshot_values(instance), Some(snapshot));
        assert_eq!(registry.read_symbol(instance, 0), 42);
        let npc = registry.npc(instance).expect("npc");
        assert_eq!(npc.id, 5);
        assert_eq!(npc.hitpoints_max, 200);
    }

    #[test]
    fn default_values_are_tracked_and_reset() {
        let mut registry = TypedRegistry::new();
        let symbol = registry.add_symbol(decl("test_int", SymbolKind::Int, false, 0, 2, None));
        registry.finish();
        registry.set_default_values(symbol, &[1, 2]);
        registry.write_values(symbol, &[1, 2]);
        assert!(registry.matches_default(symbol, &[1, 2]));
        assert!(registry.matches_default(symbol, &[1, 2, 0]));
        assert!(!registry.matches_default(symbol, &[1, 3]));

        registry.write_values(symbol, &[5, 6]);
        assert!(!registry.matches_default(symbol, &[5, 6]));
        registry.reset_to_default(symbol);
        assert_eq!(registry.snapshot_values(symbol), Some(vec![1, 2]));
        assert!(registry.matches_default(symbol, &[1, 2]));
    }

    #[test]
    fn typed_defaults_are_tracked_and_reset() {
        let (mut registry, instance) = npc_registry();
        let defaults = registry.snapshot_values(instance).expect("defaults");
        registry.set_default_values(instance, &defaults);
        assert!(registry.matches_default(instance, &defaults));

        registry.write_instance_word(instance, 288, 999);
        let changed = registry.snapshot_values(instance).expect("changed");
        assert!(!registry.matches_default(instance, &changed));
        registry.reset_to_default(instance);
        assert_eq!(registry.snapshot_values(instance), Some(defaults));
    }
}
