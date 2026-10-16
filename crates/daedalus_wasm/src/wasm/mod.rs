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
pub use wasm_binary::{BaseModule, parse_base_module};

use crate::daedalus::{CompiledDaedalusScript, Instruction, imported_functions_num};

pub(crate) const CURRENT_CLASS_INSTANCE_MEMORY_GLOBAL_INDEX: u32 = 0;
pub(crate) const CURRENT_INSTANCE_SYMBOL_INDEX: u32 = 1;
/// Global which stores number of all symbols, it is patched with real value
/// when the base module is merged with daedalus data.
pub(crate) const SYMBOLS_NUM_GLOBAL_INDEX: u32 = 3;
/// Index of `set_instance` buildin function inside base module (see `base_module.wat`),
/// converted daedalus `SetInstance` instructions call it.
pub(crate) const BASE_FUNC_SET_INSTANCE: u32 = 3;
/// Globals which store symbol indices of the well known daedalus instance
/// variables, they are patched with real values (or -1 when the script does
/// not declare them) during merge with daedalus data.
pub(crate) const SELF_SYMBOL_GLOBAL_INDEX: u32 = 4;
pub(crate) const OTHER_SYMBOL_GLOBAL_INDEX: u32 = 5;
pub(crate) const VICTIM_SYMBOL_GLOBAL_INDEX: u32 = 6;
pub(crate) const ITEM_SYMBOL_GLOBAL_INDEX: u32 = 7;
pub(crate) const HERO_SYMBOL_GLOBAL_INDEX: u32 = 8;
/// Global which stores number of all string pairs in memory, it is patched
/// with real value when the base module is merged with daedalus data.
pub(crate) const STRINGS_NUM_GLOBAL_INDEX: u32 = 9;

#[must_use]
/// Main functionality of this crate, convert compiled daedalus script file to wasm binary.
pub fn convert_daedalus_script(daedalus_binary_script_file: &Path) -> Vec<u8> {
    let script_data = CompiledDaedalusScript::from_file(daedalus_binary_script_file).unwrap();
    let base = parse_base_module(imported_functions_num(&script_data.symbols));
    let mut builder = BinaryBuilder::new(base);

    builder.add_symbols(&script_data.symbols);
    builder.add_static_strings(&script_data.static_strings);

    builder.add_test_function(&[Instruction::PushInt(0), Instruction::Return]);

    builder.finalize()
}
