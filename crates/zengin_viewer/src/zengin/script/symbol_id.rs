/// Id of a daedalus script symbol.
///
/// Valid ids are greater than 0 (0 marks an invalid symbol) and lower than
/// `i32::MAX / 2`, so a valid id can be safely converted to both `i32` and
/// `u32`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SymbolId(i32);

impl SymbolId {
    /// Exclusive upper bound of valid symbol ids
    pub const MAX: i32 = i32::MAX / 2;

    /// Creates a symbol id, `None` when the value is not a valid symbol id
    pub fn new(value: i32) -> Option<Self> {
        if value > 0 && value < Self::MAX {
            return Some(Self(value));
        }
        None
    }

    /// Raw id value, guaranteed to be in `(0, i32::MAX / 2)`
    pub fn value(self) -> i32 {
        self.0
    }

    /// Raw id value as `u32`, always positive by the validity invariant
    pub fn as_u32(self) -> u32 {
        self.0 as u32
    }
}

impl std::fmt::Display for SymbolId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
