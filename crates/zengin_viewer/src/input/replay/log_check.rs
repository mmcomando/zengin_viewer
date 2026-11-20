use bevy::prelude::*;
use regex::Regex;

use crate::input::log_capture::LogBuffer;

/// Number of trailing log lines reported when `TestLogExists` does not match.
const LOG_EXISTS_TAIL_LINES: usize = 50;

/// Replay infrastructure logs are not evidence about game behaviour: command
/// lines contain the check regex verbatim, so they would match themselves.
const IGNORED_TARGET: &str = "zengin_viewer::input::replay";

/// Panics when no buffered log line matches `pattern`.
pub(super) fn check_log_exists(log_buffer: &LogBuffer, pattern: &str) {
    let regex = match Regex::new(pattern) {
        Ok(regex) => regex,
        Err(error) => panic!("TestLogExists has an invalid regex {pattern:?}: {error}"),
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
    panic!(
        "TestLogExists did not match {pattern:?} in {} captured log line(s). Last lines:\n{tail}",
        lines.len()
    );
}

pub(super) fn matching_log_line<'a>(lines: &'a [String], regex: &Regex) -> Option<&'a str> {
    lines
        .iter()
        .filter(|line| !line.contains(IGNORED_TARGET))
        .find(|line| regex.is_match(line))
        .map(String::as_str)
}

/// Panics when a buffered log line matches `pattern`.
pub(super) fn check_log_not_exists(log_buffer: &LogBuffer, pattern: &str) {
    let regex = match Regex::new(pattern) {
        Ok(regex) => regex,
        Err(error) => panic!("TestLogNotExists has an invalid regex {pattern:?}: {error}"),
    };
    let lines = log_buffer.snapshot();
    if let Some(line) = matching_log_line(&lines, &regex) {
        panic!("TestLogNotExists matched {pattern:?} on line: {line}");
    }
    info!(
        "TestLogNotExists matched no line in {} captured log line(s)",
        lines.len()
    );
}
