//! Conversion and component execution tests.
//!
//! These live in `src` rather than `tests` so they can use crate-internal
//! items without widening the public API.

mod common;

mod fight_ai;
mod print_class_offsets;
mod test_control_flow;
mod test_manage_functions;
mod test_math_instructions;
mod test_memory;
mod test_menu_script;
mod test_real_script;
mod typed_offsets;
mod wasm_execution;
