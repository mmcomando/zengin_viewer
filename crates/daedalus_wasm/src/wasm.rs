mod binary_builder;
mod function;
mod memory;
mod wasm_binary;

use std::path::Path;

pub use binary_builder::BinaryBuilder;

pub(crate) use binary_builder::ImportedFunction;
pub(crate) use function::ConvertedFunction;
pub(crate) use memory::DaedalusMemoryLayout;
pub(crate) use wasm_binary::create_wasm_binary;

use crate::daedalus::{CompiledDaedalusScript, Instruction};

pub(crate) const CURRENT_CLASS_INSTANCE_MEMORY_GLOBAL_INDEX: u32 = 0;
pub(crate) const CURRENT_INSTANCE_SYMBOL_INDEX: u32 = 1;
/// Global which stores number of all symbols, it is patched with real value
/// when the base module is merged with daedalus data.
pub(crate) const SYMBOLS_NUM_GLOBAL_INDEX: u32 = 3;
/// Index of `set_instance` buildin function inside base module (see `base_module.wat`).
pub(crate) const BASE_FUNC_SET_INSTANCE: u32 = 3;
/// Number of buildin functions defined inside base module (see `base_module.wat`).
pub(crate) const BASE_BUILTIN_FUNCTIONS_NUM: u32 = 17;

#[must_use]
/// Main functionality of this crate, convert compiled daedalus script file to wasm binary.
pub fn convert_daedalus_script(daedalus_binary_script_file: &Path) -> Vec<u8> {
    let script_data = CompiledDaedalusScript::from_file(daedalus_binary_script_file).unwrap();
    let mut builder = BinaryBuilder::new();

    builder.add_symbols(&script_data.symbols);
    builder.add_static_strings(&script_data.static_strings);

    builder.add_test_function(&[Instruction::PushInt(0), Instruction::Return]);

    builder.finalize()
}
