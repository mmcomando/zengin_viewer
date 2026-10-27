//! Script-domain symbol ids.
//!
//! Valid ids are greater than 0 and lower than [`SymbolId::MAX`], so a valid id
//! can be safely converted to both `i32` and `u32`.

/// Id of a daedalus script symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SymbolId(i32);

impl SymbolId {
    /// Exclusive upper bound of valid symbol ids.
    pub const MAX: i32 = i32::MAX / 2;

    /// Creates a symbol id, `None` when the value is not a valid symbol id.
    #[must_use]
    pub fn new(value: i32) -> Option<Self> {
        if value > 0 && value < Self::MAX {
            return Some(Self(value));
        }
        None
    }

    /// Raw id value, guaranteed to be in `(0, i32::MAX / 2)`.
    #[must_use]
    pub fn value(self) -> i32 {
        self.0
    }
}

impl std::fmt::Display for SymbolId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

macro_rules! typed_symbol_id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(SymbolId);

        impl $name {
            /// Creates a typed symbol id, `None` when the value is not a valid
            /// symbol id.
            #[must_use]
            pub fn new(value: i32) -> Option<Self> {
                SymbolId::new(value).map(Self)
            }

            /// Wraps a symbol id whose kind was already verified to match this
            /// type, see the symbol table.
            #[must_use]
            pub fn from_symbol(symbol: SymbolId) -> Self {
                Self(symbol)
            }

            /// Underlying symbol id.
            #[must_use]
            pub fn symbol(self) -> SymbolId {
                self.0
            }

            /// Raw id value.
            #[must_use]
            pub fn value(self) -> i32 {
                self.0.value()
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fmt(f)
            }
        }
    };
}

typed_symbol_id!(
    /// Id of a daedalus script instance symbol (`C_NPC`, `C_INFO`, ...).
    InstanceId
);
typed_symbol_id!(
    /// Id of a daedalus script function symbol.
    FunctionId
);

/// Id of a daedalus script string literal. String literals are not symbols,
/// their ids are indices into the static strings table, so this type is
/// deliberately independent of [`SymbolId`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StringId(i32);

impl StringId {
    /// Exclusive upper bound of valid string ids, kept consistent with
    /// [`SymbolId::MAX`].
    pub const MAX: i32 = i32::MAX / 2;

    /// Creates a string id, `None` when the value is not a valid id. Id `0`
    /// means "unset".
    #[must_use]
    pub fn new(value: i32) -> Option<Self> {
        if value > 0 && value < Self::MAX {
            return Some(Self(value));
        }
        None
    }

    /// Raw id value.
    #[must_use]
    pub fn value(self) -> i32 {
        self.0
    }
}

impl std::fmt::Display for StringId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
