#![warn(missing_docs)]
//! Crate for converting daedalus scripts to wasm script with component model interface.

// We use as many lints as possible
// It often helps learn language and reduce bugs
// Enabled here instead of Cargo.toml so that integration tests, which are
// separate crates, don't inherit the pedantic/restriction groups
#![warn(clippy::pedantic, clippy::restriction)]
#![allow(
    clippy::blanket_clippy_restriction_lints,
    reason = "We blacklist contradicting rules"
)]
#![allow(clippy::as_conversions, reason = "to decide")]
#![allow(clippy::cast_possible_truncation, reason = "to decide")]
#![allow(clippy::if_then_some_else_none, reason = "to decide")]
#![allow(clippy::needless_pass_by_value, reason = "to decide")]
#![allow(clippy::std_instead_of_alloc, reason = "to decide")]
// Panics
#![allow(clippy::expect_used, reason = "TODO")]
#![allow(clippy::indexing_slicing, reason = "TODO")]
#![allow(clippy::panic, reason = "TODO")]
#![allow(clippy::unwrap_used, reason = "TODO")]
#![allow(clippy::get_unwrap, reason = "TODO")]
// Debug leftovers
#![allow(clippy::print_stdout, reason = "TODO")]
#![allow(clippy::uninlined_format_args, reason = "TODO")]
#![allow(clippy::use_debug, reason = "TODO")]
// Improve documentation
#![allow(clippy::missing_errors_doc, reason = "TODO")]
#![allow(clippy::missing_panics_doc, reason = "TODO")]
// Allowed lints
#![allow(
    clippy::missing_docs_in_private_items,
    reason = "Documentation for private functions is not mandatory"
)]
#![allow(
    clippy::semicolon_outside_block,
    reason = "Try to reduce semicolon after braces"
)]
#![allow(
    clippy::implicit_return,
    reason = "We use idiomatic rust which tries not to use return"
)]
#![allow(
    clippy::field_scoped_visibility_modifiers,
    reason = "Crate is small enough that pub(crate) is ok"
)]
#![allow(clippy::single_call_fn, reason = "They are good for readability")]
#![allow(
    clippy::little_endian_bytes,
    reason = "In files we save data using little endian"
)]
#![allow(
    clippy::arithmetic_side_effects,
    reason = "Handling overflows in this crate is overkill"
)]
#![allow(
    clippy::map_with_unused_argument_over_ranges,
    reason = "Code is cleaner and shorter with map"
)]
#![allow(
    clippy::integer_division_remainder_used,
    reason = "We are not security sensitive"
)]
#![allow(clippy::integer_division, reason = "We accept loss of precision")]
#![allow(clippy::pattern_type_mismatch, reason = "Makes code more ergonomic")]
#![allow(clippy::question_mark_used, reason = "Makes code more ergonomic")]
#![allow(
    clippy::default_numeric_fallback,
    reason = "Default behaviour is what we expect"
)]
#![allow(
    clippy::shadow_reuse,
    reason = "Most of the time it makes code more readable"
)]
#![allow(
    clippy::exhaustive_structs,
    reason = "Interface compatibility is not too important"
)]
#![allow(
    clippy::missing_inline_in_public_items,
    reason = "Here clear interface during debugging is more important than inlining"
)]
#![allow(clippy::pub_with_shorthand, reason = "We prefer that style")]
#![allow(clippy::pub_use, reason = "It is more ergonomic for user")]
#![allow(
    clippy::self_named_module_files,
    reason = "We don't want to have multiple mod.rs files"
)]
#![allow(
    clippy::single_char_lifetime_names,
    reason = "There is not much usage of lifetimes"
)]
#![allow(
    clippy::arbitrary_source_item_ordering,
    reason = "There are more logical orderings than alphabetical"
)]
#![allow(
    clippy::separated_literal_suffix,
    reason = "We prefer literal suffixes separated by underscore"
)]

pub(crate) mod common;

/// Representation of daedalus scripts data in rust types.
pub mod daedalus;
/// Functionality to convert daedalus to wasm.
pub mod wasm;
