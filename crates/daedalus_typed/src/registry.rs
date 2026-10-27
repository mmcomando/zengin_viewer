//! Typed script-class registry and VM access dispatch.

use std::sync::{Arc, PoisonError, RwLock};

use super::access::ScriptVmAccess;
use super::generic::GenericVariable;
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
}

#[derive(Debug, Clone)]
struct SymbolEntry {
    name: String,
    kind: SymbolKind,
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
    /// Value slot word 0 of instance symbols. For classes whose members start
    /// past offset 0 it holds the instance reference (for example `self`/`other`
    /// values); for other classes it is unused.
    references: Vec<i32>,
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

        let storage = if decl.is_const || decl.is_class_member {
            None
        } else {
            GenericVariable::new(decl.kind, decl.element_count).map(TypedInstance::Generic)
        };

        self.symbols.push(SymbolEntry {
            name: decl.name.to_owned(),
            kind: decl.kind,
            is_class_member: decl.is_class_member,
            address_or_offset: decl.address_or_offset,
            element_count: decl.element_count,
            has_function: decl.has_function,
            parent: decl.parent,
        });
        self.instances.push(storage);
        self.references.push(0);
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
            .get(usize::try_from(instance).ok()?)?
            .as_ref()
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

    /// Writes element `element` of a global/variable symbol.
    pub fn write_symbol(&mut self, symbol: u32, element: u32, value: i32) {
        let symbol_index = usize::try_from(symbol).unwrap_or(usize::MAX);
        let reference_class = self
            .instances
            .get(symbol_index)
            .and_then(Option::as_ref)
            .and_then(TypedInstance::class);
        if element == 0
            && reference_class.is_some_and(ClassKind::uses_reference_word)
            && let Some(reference) = self.references.get_mut(symbol_index)
        {
            *reference = value;
            return;
        }
        let Some(instance) = self
            .instances
            .get_mut(symbol_index)
            .and_then(Option::as_mut)
        else {
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
            self.write_symbol(symbol, element, *value);
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
        let Some(storage) = self
            .instances
            .get_mut(usize::try_from(instance).unwrap_or(usize::MAX))
            .and_then(Option::as_mut)
        else {
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
        let mut current = symbol;
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
}
