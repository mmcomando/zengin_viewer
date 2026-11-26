//! Small developer-facing utilities shared by the zengin_viewer crates.
//!
//! - [`perf_debug`] opt-in per-function timing and counter logging, gated behind
//!   the `debug_perf` feature and a no-op without it.
//! - [`test_log!`] a stable `TEST_LOG` log target for scenario assertions.
//! - [`warn_once!`] and [`warn_unimplemented!`] log a warning only the first
//!   time the call site runs.
//! - [`amortized_executor`] runs a set of callbacks on a virtual clock under a
//!   per-frame work budget.
//! - [`external_messages`] bridges messages produced outside the ECS into the
//!   regular Bevy message buffers.
//! - [`ui_kit`] small Bevy UI pieces shared by the overlay screens.
//! - [`updater`] periodic entity update streams driven by the amortized
//!   executor.
//! - [`wav_duration`] reads the play time out of a wav byte buffer.

pub mod amortized_executor;
pub mod external_messages;
mod logging;
pub mod perf_debug;
pub mod ui_kit;
pub mod updater;
mod wav;

pub use crate::wav::wav_duration;
