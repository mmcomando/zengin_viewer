//! Generic host-side storage of daedalus symbol values.
//!

#![allow(dead_code)]
//! Converted script functions no longer keep global variables and class members
//! in a wasm memory. They call the `symbols` interface imports instead, so this
//! store is the only copy of the mutable script state.
//!
//! The store is deliberately free of game semantics: it knows symbol metadata,
//! names and a flat word space. Typed class models live in `daedalus_typed`.

use std::collections::HashMap;

use crate::daedalus::Symbol;
use crate::daedalus::SymbolKind;

/// Metadata of one daedalus symbol, mirroring the `symbol-info` guest record.
#[derive(Debug, Clone, Copy)]
pub struct SymbolMeta {
    /// Kind of the symbol.
    pub kind: SymbolKind,
    /// Const globals are readable through their value slot but reject writes.
    pub is_const: bool,
    /// Class members live inside their parent instance record, the
    /// `address_or_offset` field is then a byte offset, not a slot base.
    pub is_class_member: bool,
    /// Global symbols: byte base of the value slot. Class members: byte offset
    /// inside the instance record.
    pub address_or_offset: u32,
    /// Number of 32bit values the value slot holds, `0` for class members.
    /// Const globals keep their declared count.
    pub element_count: u32,
    /// Whether the symbol has a wasm function with code.
    pub has_function: bool,
}

/// Symbol metadata plus the symbol name and parent, needed to resolve class
/// members by name.
#[derive(Debug, Clone)]
struct SymbolEntry {
    meta: SymbolMeta,
    name: String,
    parent: Option<u32>,
}

/// One word space holding every daedalus symbol value, laid out exactly like
/// the old wasm symbol memory: global symbols get a contiguous region, instance
/// records are regions addressed by their class member offsets.
#[derive(Debug, Default, Clone)]
pub struct SymbolStore {
    words: Vec<i32>,
    symbols: Vec<SymbolEntry>,
    /// Class symbol id by lowercased class name.
    class_by_name: HashMap<String, u32>,
    /// Member symbol id by `(parent class id, lowercased member name)`.
    class_members: HashMap<(u32, String), u32>,
}

impl SymbolStore {
    /// Creates an empty store with the given word capacity.
    #[must_use]
    pub fn new(word_count: usize) -> Self {
        Self {
            words: vec![0; word_count],
            ..Self::default()
        }
    }

    /// Creates a store from parsed daedalus symbols, with their default values
    /// already applied. Tests use this constructor.
    #[must_use]
    pub fn from_symbols(symbols: &[Symbol]) -> Self {
        let mut word_count = 0_u32;
        let mut entries = Vec::with_capacity(symbols.len());
        for symbol in symbols {
            let (is_class_member, address_or_offset, element_count) = if symbol.is_class_var {
                (true, symbol.offset.unwrap_or(0), symbol.elements_count)
            } else {
                let address = word_count * 4;
                word_count += symbol.elements_count;
                (false, address, symbol.elements_count)
            };
            entries.push(SymbolEntry {
                meta: SymbolMeta {
                    kind: symbol.kind,
                    is_const: symbol.is_const,
                    is_class_member,
                    address_or_offset,
                    element_count,
                    has_function: symbol.is_function_with_code(),
                },
                name: symbol.name.clone(),
                parent: symbol.parent,
            });
        }

        let mut store = Self {
            words: vec![0; usize::try_from(word_count).unwrap_or_default()],
            symbols: entries,
            ..Self::default()
        };

        for (index, symbol) in symbols.iter().enumerate() {
            if symbol.is_class_var {
                continue;
            }
            let base = usize::try_from(store.symbols[index].meta.address_or_offset / 4)
                .unwrap_or_default();
            let count = usize::try_from(symbol.elements_count).unwrap_or_default();
            for (element, value) in symbol.default_values.iter().take(count).enumerate() {
                if let Some(word) = store.words.get_mut(base + element) {
                    *word = *value;
                }
            }
        }

        store.build_name_index();
        store
    }

    /// Number of symbols the store knows about.
    #[must_use]
    pub fn symbol_count(&self) -> usize {
        self.symbols.len()
    }

    /// Number of words in the value space.
    #[must_use]
    pub fn word_count(&self) -> usize {
        self.words.len()
    }

    /// Metadata of a symbol, `None` when the id is out of range.
    #[must_use]
    pub fn meta(&self, symbol: u32) -> Option<&SymbolMeta> {
        self.symbols
            .get(usize::try_from(symbol).ok()?)
            .map(|entry| &entry.meta)
    }

    /// Name of a symbol, `None` when the id is out of range.
    #[must_use]
    pub fn name(&self, symbol: u32) -> Option<&str> {
        self.symbols
            .get(usize::try_from(symbol).ok()?)
            .map(|entry| entry.name.as_str())
    }

    /// Parent (class or prototype) of a symbol.
    #[must_use]
    pub fn parent(&self, symbol: u32) -> Option<u32> {
        self.symbols.get(usize::try_from(symbol).ok()?)?.parent
    }

    /// Appends a symbol loaded from the guest metadata. Call
    /// [`Self::build_name_index`] after all symbols are loaded.
    pub fn push_symbol(&mut self, meta: SymbolMeta, name: String, parent: Option<u32>) {
        self.symbols.push(SymbolEntry { meta, name, parent });
    }

    /// Builds the class name and member name indices.
    pub fn build_name_index(&mut self) {
        self.class_by_name.clear();
        self.class_members.clear();
        for (index, entry) in self.symbols.iter().enumerate() {
            let id = u32::try_from(index).unwrap_or(u32::MAX);
            if entry.meta.kind == SymbolKind::Class {
                self.class_by_name
                    .insert(entry.name.to_ascii_lowercase(), id);
            }
            if entry.meta.is_class_member {
                let Some(parent) = entry.parent else {
                    continue;
                };
                let member_name = entry
                    .name
                    .rsplit_once('.')
                    .map_or(entry.name.as_str(), |(_, member)| member)
                    .to_ascii_lowercase();
                self.class_members.insert((parent, member_name), id);
            }
        }
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

    /// Writes element `element` of a global/variable symbol value. Const
    /// symbols keep their compile-time value and reject writes.
    pub fn write_symbol(&mut self, symbol: u32, element: u32, value: i32) -> bool {
        if self
            .symbols
            .get(symbol as usize)
            .is_some_and(|entry| entry.meta.is_const)
        {
            return false;
        }
        let Some(slot) = self.value_slot(symbol, element) else {
            return false;
        };
        let Some(word) = self.words.get_mut(slot) else {
            return false;
        };
        *word = value;
        true
    }

    /// Reads element `element` of the class member with symbol `member` of
    /// `instance` from the raw word space.
    #[must_use]
    pub fn read_member(&self, instance: u32, member: u32, element: u32) -> Option<i32> {
        let slot = self.member_slot(instance, member, element)?;
        self.words.get(slot).copied()
    }

    /// Writes element `element` of the class member with symbol `member` of
    /// `instance` into the raw word space.
    pub fn write_member(&mut self, instance: u32, member: u32, element: u32, value: i32) -> bool {
        let Some(slot) = self.member_slot(instance, member, element) else {
            return false;
        };
        let Some(word) = self.words.get_mut(slot) else {
            return false;
        };
        *word = value;
        true
    }

    /// Reads the class member word at byte `offset` of `instance`.
    #[must_use]
    pub fn read_offset(&self, instance: u32, offset: u32) -> Option<i32> {
        let meta = self.meta(instance)?;
        if meta.is_class_member {
            return None;
        }
        let base = usize::try_from(meta.address_or_offset / 4).ok()?;
        let offset = usize::try_from(offset / 4).ok()?;
        self.words.get(base + offset).copied()
    }

    /// Writes the class member word at byte `offset` of `instance`.
    pub fn write_offset(&mut self, instance: u32, offset: u32, value: i32) -> bool {
        let Some(meta) = self.meta(instance) else {
            return false;
        };
        if meta.is_class_member {
            return false;
        }
        let Some(base) = usize::try_from(meta.address_or_offset / 4).ok() else {
            return false;
        };
        let Some(offset) = usize::try_from(offset / 4).ok() else {
            return false;
        };
        let Some(word) = self.words.get_mut(base + offset) else {
            return false;
        };
        *word = value;
        true
    }

    /// Member symbol id of a named class member.
    #[must_use]
    pub fn find_member(&self, class_name: &str, member_name: &str) -> Option<u32> {
        let class_id = self
            .class_by_name
            .get(&class_name.to_ascii_lowercase())
            .copied()?;
        self.class_members
            .get(&(class_id, member_name.to_ascii_lowercase()))
            .copied()
    }

    /// Class symbol id by name, case-insensitive.
    #[must_use]
    pub fn class_symbol(&self, class_name: &str) -> Option<u32> {
        self.class_by_name
            .get(&class_name.to_ascii_lowercase())
            .copied()
    }

    /// Walks the parent chain of an instance/prototype symbol until a class
    /// symbol is found.
    #[must_use]
    pub fn class_of(&self, symbol: u32) -> Option<u32> {
        let mut current = symbol;
        // Prototypes can form a chain, the class is somewhere at its end.
        for _ in 0..16 {
            let entry = self.symbols.get(usize::try_from(current).ok()?)?;
            if entry.meta.kind == SymbolKind::Class {
                return Some(current);
            }
            current = entry.parent?;
        }
        None
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

    fn member_slot(&self, instance: u32, member: u32, element: u32) -> Option<usize> {
        let instance_meta = self.meta(instance)?;
        if instance_meta.is_class_member {
            return None;
        }
        let member_meta = self.meta(member)?;
        if !member_meta.is_class_member {
            return None;
        }
        let base = usize::try_from(instance_meta.address_or_offset / 4).ok()?;
        let offset = usize::try_from(member_meta.address_or_offset / 4).ok()?;
        let element = usize::try_from(element).ok()?;
        Some(base + offset + element)
    }
}
