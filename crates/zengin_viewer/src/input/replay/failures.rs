//! Collected failures of the `Test*` scenario commands.

use bevy::prelude::*;
use dev_util::perf_debug;

/// Failures of the `Test*` scenario commands. They are collected instead of
/// panicking at the failing command, so the rest of the scenario still runs and
/// its logs are gathered before the test result is reported at exit.
#[derive(Resource, Default)]
pub(super) struct TestFailures(Vec<String>);

impl TestFailures {
    pub(super) fn push(&mut self, message: String) {
        // The target is under `input::replay`, so the log checks themselves
        // ignore this line like the other replay infrastructure lines.
        error!("{message}");
        self.0.push(message);
    }

    pub(super) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Reports the collected failures as the scenario exit result.
    pub(super) fn panic_if_any(&self) {
        if self.0.is_empty() {
            return;
        }
        panic!(
            "{} scenario test failure(s):\n\n{}",
            self.0.len(),
            self.0.join("\n\n")
        );
    }
}

/// Reports collected failures when the app exits without an explicit
/// `ExitGame`, for example when the scenario frame limit is reached.
pub(super) fn report_failures_on_exit(failures: Res<TestFailures>, exits: MessageReader<AppExit>) {
    let _perf = perf_debug!();
    if failures.is_empty() || exits.is_empty() {
        return;
    }
    failures.panic_if_any();
}

#[cfg(test)]
mod tests {
    use super::TestFailures;

    #[test]
    fn empty_failures_do_not_panic() {
        TestFailures::default().panic_if_any();
    }

    #[test]
    #[should_panic(expected = "2 scenario test failure(s)")]
    fn collected_failures_panic_with_their_count() {
        let mut failures = TestFailures::default();
        failures.push("first".to_string());
        failures.push("second".to_string());

        failures.panic_if_any();
    }
}
