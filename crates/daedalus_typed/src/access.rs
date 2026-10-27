//! Access and conversion traits for G2 script classes.

/// Maps raw daedalus class member accesses to typed Rust fields.
pub trait ScriptVmAccess {
    /// Reads element `element` of the member at byte `offset`, `None` when the
    /// offset/element is not modeled by this class.
    fn read(&self, offset: u32, element: u32) -> Option<i32>;

    /// Writes element `element` of the member at byte `offset`, `false` when
    /// not modeled (the registry reports the access).
    fn write(&mut self, offset: u32, element: u32, value: i32) -> bool;
}

/// Converts a raw daedalus integer to a typed enum value and back.
///
/// Implementations must preserve unknown values (usually with an
/// `Other(i32)` variant) so script writes never lose data.
pub trait ScriptConversion: Sized {
    /// Converts a raw value to the typed representation.
    fn from_i32(value: i32) -> Self;

    /// Converts the typed value back to its raw representation.
    fn to_i32(self) -> i32;
}
