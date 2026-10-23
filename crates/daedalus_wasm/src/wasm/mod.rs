mod binary_builder;
mod constants;
mod convert;
mod function;
mod memory;
mod wasm_binary;

pub(crate) use binary_builder::ImportedFunction;
pub(crate) use function::ConvertedFunction;
pub(crate) use memory::DaedalusMemoryLayout;
pub(crate) use wasm_binary::create_wasm_binary;

pub use binary_builder::BinaryBuilder;
pub use convert::convert_daedalus_script;
pub use wasm_binary::{BaseModule, parse_base_module};
