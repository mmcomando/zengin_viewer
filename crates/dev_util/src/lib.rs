//! Small developer-facing utilities shared by the zengin_viewer crates.
//!
//! - [`perf_debug`] opt-in per-function timing and counter logging, gated behind
//!   the `debug_perf` feature and a no-op without it.
//! - [`test_log!`] a stable `TEST_LOG` log target for scenario assertions.
//! - [`warn_once!`] and [`warn_unimplemented!`] log a warning only the first
//!   time the call site runs.
//! - [`amortized_executor`] runs a set of callbacks on a virtual clock under a
//!   per-frame work budget.

pub mod amortized_executor;
mod logging;
pub mod perf_debug;

// The logging macros expand in the caller's crate, so the compiler never sees a
// direct use of `bevy` here without the `debug_perf` feature. Keep the
// dependency referenced so Cargo's unused-dependency check stays quiet.
#[allow(unused_imports)]
use bevy as _;
