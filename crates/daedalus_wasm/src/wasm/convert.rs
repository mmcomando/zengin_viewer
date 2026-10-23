use crate::{
    daedalus::{CompiledDaedalusScript, Instruction, imported_functions_num},
    wasm::{BinaryBuilder, parse_base_module},
};
use std::path::Path;

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
