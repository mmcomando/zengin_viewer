pub(crate) mod analyze;
mod instruction;
mod script;

pub use instruction::Instruction;

pub(crate) use script::CallSignature;
pub(crate) use script::CompiledDaedalusScript;
pub(crate) use script::Function;
pub(crate) use script::Symbol;
pub(crate) use script::SymbolKind;
pub(crate) use script::imported_functions_num;

#[cfg(test)]
pub(crate) use script::external_functions;
