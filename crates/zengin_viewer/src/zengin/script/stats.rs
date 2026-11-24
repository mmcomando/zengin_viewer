//! Per-frame debug statistics of host-initiated script function calls.

#[cfg(feature = "debug_perf")]
use super::state::SymbolTable;
use super::symbol_id::FunctionId;
#[cfg(feature = "debug_perf")]
use bevy::log::info;
#[cfg(feature = "debug_perf")]
use std::collections::HashMap;

/// Counts host-initiated script function calls since the previous flush.
#[cfg(feature = "debug_perf")]
#[derive(Debug, Default)]
pub(crate) struct ScriptCallStats {
    counts: HashMap<FunctionId, u64>,
}

#[cfg(feature = "debug_perf")]
impl ScriptCallStats {
    pub(crate) fn record(&mut self, function: FunctionId) {
        *self.counts.entry(function).or_default() += 1;
    }

    /// Logs the counts accumulated since the previous flush, most called first.
    /// Functions are resolved through `symbols`, unknown names are shown as
    /// their raw id. Silent when no function was called.
    pub(crate) fn flush(&mut self, symbols: &SymbolTable) {
        if self.counts.is_empty() {
            return;
        }
        let mut entries = self
            .counts
            .drain()
            .map(|(function, count)| {
                let name = symbols
                    .symbol_name(function.value())
                    .unwrap_or_else(|| format!("#{}", function.value()));
                (name, count)
            })
            .collect::<Vec<_>>();
        entries.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));

        let total = entries.iter().map(|(_, count)| count).sum::<u64>();
        info!(
            target: "script_stats",
            "ScriptCallStats: total_calls({total}), functions({})",
            entries.len()
        );
        for (name, count) in entries {
            info!(target: "script_stats", "ScriptCallStats: {name}({count})");
        }
    }
}

/// No-op statistics when the `debug_perf` feature is disabled.
#[cfg(not(feature = "debug_perf"))]
#[derive(Debug, Default)]
pub(crate) struct ScriptCallStats;

#[cfg(not(feature = "debug_perf"))]
impl ScriptCallStats {
    #[inline]
    pub(crate) fn record(&mut self, _function: FunctionId) {}
}
