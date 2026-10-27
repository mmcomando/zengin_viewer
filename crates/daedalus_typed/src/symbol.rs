//! Daedalus symbol metadata used by the host-side registry.

/// Kind of a daedalus symbol, matching the `.DAT` format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    /// Class.
    Class,
    /// Float.
    Float,
    /// Func.
    Func,
    /// Instance.
    Instance,
    /// Int.
    Int,
    /// Prototype.
    Prototype,
    /// String.
    String,
    /// Variable argument.
    VariableArgument,
}

/// One symbol declaration loaded from the guest metadata.
#[derive(Debug, Clone, Copy)]
pub struct SymbolDecl<'a> {
    /// Symbol name as stored in the script binary.
    pub name: &'a str,
    /// Symbol kind.
    pub kind: SymbolKind,
    /// Const symbols are inlined at compile time and have no value slot.
    pub is_const: bool,
    /// Class members live inside their parent instance record.
    pub is_class_member: bool,
    /// Global symbols: byte base of the value slot. Class members: byte offset
    /// inside the parent instance record.
    pub address_or_offset: u32,
    /// Number of 32-bit words the value slot holds.
    pub element_count: u32,
    /// Whether the symbol has wasm function code.
    pub has_function: bool,
    /// Parent symbol id, for class members and instances.
    pub parent: Option<u32>,
}
