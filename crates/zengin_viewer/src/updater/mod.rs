//! Amortized periodic callback execution.

mod executor;
mod periodic;
mod plugin;

pub(crate) use periodic::{Periodic, PeriodicAppExt, PeriodicKind, PeriodicUpdate};
pub(crate) use plugin::AmortizedExecutorPlugin;
