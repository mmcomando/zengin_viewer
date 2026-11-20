use bevy::prelude::*;
use regex::Regex;

use crate::input::log_capture::LogBuffer;

use super::failures::TestFailures;

/// Number of trailing log lines reported when `TestLogExists` does not match.
const LOG_EXISTS_TAIL_LINES: usize = 50;

/// Replay infrastructure logs are not evidence about game behaviour: command
/// lines contain the check regex verbatim, so they would match themselves.
const IGNORED_TARGET: &str = "zengin_viewer::input::replay";

/// Records a `TestLogExists` failure in the test result instead of aborting the
/// scenario, so later commands and their logs still run.
pub(super) fn check_log_exists(log_buffer: &LogBuffer, pattern: &str, failures: &mut TestFailures) {
    let regex = match Regex::new(pattern) {
        Ok(regex) => regex,
        Err(error) => {
            failures.push(format!(
                "TestLogExists has an invalid regex {pattern:?}: {error}"
            ));
            return;
        }
    };
    let lines = log_buffer.snapshot();
    if let Some(line) = matching_log_line(&lines, &regex) {
        info!("TestLogExists matched {pattern:?} on line: {line}");
        return;
    }
    let tail = lines
        .iter()
        .rev()
        .take(LOG_EXISTS_TAIL_LINES)
        .rev()
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    failures.push(format!(
        "TestLogExists did not match {pattern:?} in {} captured log line(s). Last lines:\n{tail}",
        lines.len()
    ));
}

pub(super) fn matching_log_line<'a>(lines: &'a [String], regex: &Regex) -> Option<&'a str> {
    lines
        .iter()
        .filter(|line| !line.contains(IGNORED_TARGET))
        .find(|line| regex.is_match(line))
        .map(String::as_str)
}

/// Records a `TestLogNotExists` failure in the test result instead of aborting
/// the scenario.
pub(super) fn check_log_not_exists(
    log_buffer: &LogBuffer,
    pattern: &str,
    failures: &mut TestFailures,
) {
    let regex = match Regex::new(pattern) {
        Ok(regex) => regex,
        Err(error) => {
            failures.push(format!(
                "TestLogNotExists has an invalid regex {pattern:?}: {error}"
            ));
            return;
        }
    };
    let lines = log_buffer.snapshot();
    if let Some(line) = matching_log_line(&lines, &regex) {
        failures.push(format!(
            "TestLogNotExists matched {pattern:?} on line: {line}"
        ));
        return;
    }
    info!(
        "TestLogNotExists matched no line in {} captured log line(s)",
        lines.len()
    );
}

#[cfg(test)]
mod tests {
    use super::{check_log_exists, check_log_not_exists};
    use crate::input::log_capture::LogBuffer;
    use crate::input::replay::failures::TestFailures;

    #[test]
    fn missing_log_line_is_collected_as_a_failure() {
        let buffer = LogBuffer::default();
        buffer.push("INFO zengin_viewer: something".to_string());
        let mut failures = TestFailures::default();

        check_log_exists(&buffer, "never logged", &mut failures);

        assert!(!failures.is_empty());
    }

    #[test]
    fn unexpected_log_line_is_collected_as_a_failure() {
        let buffer = LogBuffer::default();
        buffer.push("INFO zengin_viewer: something".to_string());
        let mut failures = TestFailures::default();

        check_log_not_exists(&buffer, "something", &mut failures);

        assert!(!failures.is_empty());
    }
}
