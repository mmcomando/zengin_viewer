//! Opt-in per-function performance logging.
//!
//! Wrap a function body with [`perf_debug!`](crate::perf_debug!) and the guard
//! logs the caller function, its wall time and any counters recorded on it when
//! the guard is dropped:
//!
//! ```ignore
//! fn move_npcs(npcs: Query<Entity>) {
//!     let perf = perf_debug!();
//!     perf.record_count("npcs", npcs.iter().count());
//!     // ...
//! }
//! // PerfDebug: func(gameplay::character::move_npcs), time_us(1200), npcs(120)
//! ```
//!
//! Everything is gated behind the `debug_perf` cargo feature. Without it the
//! guard and all of its methods are no-ops that the optimizer removes. With it,
//! [`PerfDebugPlugin`] also logs a `frame(...)` marker at the start of every
//! frame so the log can be split into per-frame sections.

#[cfg(feature = "debug_perf")]
use bevy::prelude::*;
#[cfg(feature = "debug_perf")]
use std::cell::RefCell;
#[cfg(feature = "debug_perf")]
use std::fmt::Write;
#[cfg(feature = "debug_perf")]
use std::time::Instant;

/// Measures a call until the guard is dropped, then logs one line.
///
/// Create it with [`perf_debug!`](crate::perf_debug!) so the function name is
/// filled in automatically.
#[cfg(feature = "debug_perf")]
#[must_use]
pub struct PerfDebug {
    name: &'static str,
    start: Instant,
    fields: RefCell<Vec<(&'static str, u64)>>,
}

#[cfg(feature = "debug_perf")]
impl PerfDebug {
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            start: Instant::now(),
            fields: RefCell::new(Vec::new()),
        }
    }

    /// Records a named counter, replacing any previous value for that name.
    pub fn record_count(&self, name: &'static str, value: usize) {
        self.set(name, value as u64);
    }

    /// Adds `delta` to a named counter, creating it at `delta` when absent.
    pub fn increment_tag(&self, name: &'static str, delta: u64) {
        let mut fields = self.fields.borrow_mut();
        if let Some((_, value)) = fields.iter_mut().find(|(field, _)| *field == name) {
            *value += delta;
            return;
        }
        fields.push((name, delta));
    }

    fn set(&self, name: &'static str, value: u64) {
        let mut fields = self.fields.borrow_mut();
        if let Some((_, field_value)) = fields.iter_mut().find(|(field, _)| *field == name) {
            *field_value = value;
            return;
        }
        fields.push((name, value));
    }
}

#[cfg(feature = "debug_perf")]
impl Drop for PerfDebug {
    fn drop(&mut self) {
        let mut line = format!(
            "PerfDebug: func({}), time_us({})",
            self.name,
            self.start.elapsed().as_micros()
        );
        for (name, value) in self.fields.get_mut() {
            // Writing into a `String` is infallible.
            let _write_result = write!(line, ", {name}({value})");
        }
        bevy::log::info!(target: "perf_debug", "{line}");
    }
}

/// No-op guard when the `debug_perf` feature is disabled.
#[cfg(not(feature = "debug_perf"))]
#[must_use]
pub struct PerfDebug;

#[cfg(not(feature = "debug_perf"))]
#[allow(clippy::unused_self)] // The no-op methods keep the same API as the enabled guard.
impl PerfDebug {
    #[inline]
    pub fn new(_name: &'static str) -> Self {
        Self
    }

    #[inline]
    pub fn record_count(&self, _name: &'static str, _value: usize) {}

    #[inline]
    pub fn increment_tag(&self, _name: &'static str, _delta: u64) {}
}

/// Creates a [`PerfDebug`] guard named after the enclosing function.
///
/// The guard measures until it is dropped, so bind it for the whole scope:
/// `let perf = perf_debug!();`. Bind it as `_perf` when no counters are recorded.
#[macro_export]
macro_rules! perf_debug {
    () => {{
        #[cfg(feature = "debug_perf")]
        let name = {
            fn __perf_debug_scope() {}
            fn __perf_debug_type_name<T>(_: T) -> &'static str {
                ::std::any::type_name::<T>()
            }
            let full = __perf_debug_type_name(__perf_debug_scope);
            let name = full.strip_suffix("::__perf_debug_scope").unwrap_or(full);
            // Drop the leading crate segment so the log shows `module::path::fn`
            // without the caller's crate name.
            match name.split_once("::") {
                Some((_crate_root, rest)) => rest,
                None => name,
            }
        };
        #[cfg(not(feature = "debug_perf"))]
        let name = "";
        $crate::perf_debug::PerfDebug::new(name)
    }};
}

/// Logs a `frame(...)` marker at the start of every frame when `debug_perf` is
/// enabled, so a script can split the log into per-frame sections.
#[cfg(feature = "debug_perf")]
pub struct PerfDebugPlugin;

#[cfg(feature = "debug_perf")]
impl Plugin for PerfDebugPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(First, log_frame_start);
    }
}

#[cfg(feature = "debug_perf")]
fn log_frame_start(time: Res<Time>, mut frame: Local<u64>) {
    *frame += 1;
    bevy::log::info!(
        target: "perf_debug",
        "PerfDebug: frame({}), elapsed_us({})",
        *frame,
        time.elapsed().as_micros()
    );
}
