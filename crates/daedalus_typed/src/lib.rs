#![warn(missing_docs)]
//! Semantic Rust models of the G2 daedalus script classes the host uses.
//!
//! Each class maps raw `(byte offset, element)` accesses from converted script
//! code to named, typed Rust fields. Only fields the host or the scripts
//! actually use are modeled; everything else stays in the generic word store of
//! `daedalus_wasm`, so unmodeled script accesses keep working.
//!
//! The hardcoded G2 offsets are validated against the loaded script at registry
//! build time. A mismatch disables typing for that class and is reported as a
//! [`ValidationIssue`].

mod access;
pub mod ids;
mod info;
mod item;
mod menu;
mod menu_item;
mod npc;
mod registry;

pub use access::{ScriptConversion, ScriptVmAccess};
pub use info::Info;
pub use item::Item;
pub use menu::Menu;
pub use menu_item::{MenuItem, MenuItemKind};
pub use npc::Npc;
pub use registry::{ClassKind, MemberAccess, TypedInstance, TypedRegistry, ValidationIssue};
