//! Primitive daedalus symbol values without a typed class model.

use super::ids::StringId;
use super::symbol::SymbolKind;

/// Raw word storage of a primitive symbol or of an instance record without a
/// typed class model.
#[derive(Debug, Clone)]
pub struct GenericVariable {
    kind: SymbolKind,
    words: Vec<i32>,
}

impl GenericVariable {
    /// Creates zeroed storage of `element_count` words, `None` for symbol kinds
    /// without a value slot.
    #[must_use]
    pub fn new(kind: SymbolKind, element_count: u32) -> Option<Self> {
        if !has_storage(kind) {
            return None;
        }
        let words = usize::try_from(element_count).unwrap_or_default();
        Some(Self {
            kind,
            words: vec![0; words],
        })
    }

    /// Symbol kind the storage belongs to.
    #[must_use]
    pub fn kind(&self) -> SymbolKind {
        self.kind
    }

    /// Number of stored words.
    #[must_use]
    pub fn len(&self) -> usize {
        self.words.len()
    }

    /// Whether the storage has no words.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
    }

    /// Raw word at `element`.
    #[must_use]
    pub fn get(&self, element: u32) -> Option<i32> {
        self.words.get(usize::try_from(element).ok()?).copied()
    }

    /// Writes a raw word, growing the storage when `element` is past its end.
    pub fn set(&mut self, element: u32, value: i32) -> bool {
        let Ok(index) = usize::try_from(element) else {
            return false;
        };
        if self.words.len() <= index {
            self.words.resize(index + 1, 0);
        }
        self.words[index] = value;
        true
    }

    /// Grows the storage to at least `words` entries.
    pub fn resize(&mut self, words: usize) {
        if self.words.len() < words {
            self.words.resize(words, 0);
        }
    }

    /// Raw words.
    #[must_use]
    pub fn words(&self) -> &[i32] {
        &self.words
    }

    /// Scalar int value, `None` for other kinds or non-scalar storage.
    #[must_use]
    pub fn as_int(&self) -> Option<i32> {
        (self.kind == SymbolKind::Int && self.words.len() == 1).then(|| self.words[0])
    }

    /// Int words, `None` for other kinds.
    #[must_use]
    pub fn as_ints(&self) -> Option<&[i32]> {
        (self.kind == SymbolKind::Int).then_some(&self.words)
    }

    /// Scalar float value, `None` for other kinds or non-scalar storage.
    #[must_use]
    pub fn as_float(&self) -> Option<f32> {
        (self.kind == SymbolKind::Float && self.words.len() == 1)
            .then(|| f32::from_bits(self.words[0] as u32))
    }

    /// Float values decoded from their bit patterns, `None` for other kinds.
    #[must_use]
    pub fn as_floats(&self) -> Option<impl Iterator<Item = f32> + '_> {
        (self.kind == SymbolKind::Float)
            .then(|| self.words.iter().map(|word| f32::from_bits(*word as u32)))
    }

    /// Scalar string id, `None` for other kinds, non-scalar storage or an
    /// unset (`0`) id.
    #[must_use]
    pub fn as_string(&self) -> Option<StringId> {
        (self.kind == SymbolKind::String && self.words.len() == 1)
            .then(|| StringId::new(self.words[0]))
            .flatten()
    }

    /// String ids, skipping unset (`0`) entries, `None` for other kinds.
    #[must_use]
    pub fn as_strings(&self) -> Option<impl Iterator<Item = StringId> + '_> {
        (self.kind == SymbolKind::String)
            .then(|| self.words.iter().filter_map(|word| StringId::new(*word)))
    }
}

fn has_storage(kind: SymbolKind) -> bool {
    matches!(
        kind,
        SymbolKind::Float
            | SymbolKind::Instance
            | SymbolKind::Int
            | SymbolKind::Prototype
            | SymbolKind::String
    )
}
