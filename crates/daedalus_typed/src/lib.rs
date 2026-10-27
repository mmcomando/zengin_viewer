#![warn(missing_docs)]
//! Semantic Rust models of the G2 daedalus script classes the host uses.
//!
//! Each class maps raw `(byte offset, element)` accesses from converted script
//! code to named, typed Rust fields. Only fields the host or the scripts
//! actually use are modeled; primitives and unmodeled classes use generic word
//! storage in [`TypedRegistry`].
//!
//! The hardcoded G2 offsets are validated against the loaded script at registry
//! build time. A mismatch panics, because the compiled accessors would otherwise
//! silently read and write the wrong fields.

mod access;
mod generic;
pub mod ids;
mod info;
mod item;
mod menu;
mod menu_item;
mod npc;
mod registry;
mod symbol;

pub use access::{ScriptConversion, ScriptVmAccess};
pub use generic::GenericVariable;
pub use info::Info;
pub use item::Item;
pub use menu::Menu;
pub use menu_item::{MenuItem, MenuItemKind};
pub use npc::Npc;
pub use registry::{ClassKind, TypedInstance, TypedRegistry};
pub use symbol::{SymbolDecl, SymbolKind};
