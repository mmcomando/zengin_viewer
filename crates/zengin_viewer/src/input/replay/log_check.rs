use bevy::prelude::*;
use regex::Regex;

use crate::input::log_capture::LogBuffer;

/// Number of trailing log lines reported when `LogExists` does not match.
const LOG_EXISTS_TAIL_LINES: usize = 50;

/// Panics when no buffered log line matches `pattern`.
pub(super) fn check_log_exists(log_buffer: &LogBuffer, pattern: &str) {
    let regex = match Regex::new(pattern) {
        Ok(regex) => regex,
        Err(error) => panic!("LogExists has an invalid regex {pattern:?}: {error}"),
    };
    let lines = log_buffer.snapshot();
    if let Some(line) = matching_log_line(&lines, &regex) {
        info!("LogExists matched {pattern:?} on line: {line}");
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
        "LogExists did not match {pattern:?} in {} captured log line(s). Last lines:\n{tail}",
        lines.len()
    );
}

pub(super) fn matching_log_line<'a>(lines: &'a [String], regex: &Regex) -> Option<&'a str> {
    lines
        .iter()
        .find(|line| regex.is_match(line))
        .map(String::as_str)
}

/// Panics when a buffered log line matches `pattern`.
pub(super) fn check_log_not_exists(log_buffer: &LogBuffer, pattern: &str) {
    let regex = match Regex::new(pattern) {
        Ok(regex) => regex,
        Err(error) => panic!("LogNotExists has an invalid regex {pattern:?}: {error}"),
    };
    let lines = log_buffer.snapshot();
    if let Some(line) = matching_log_line(&lines, &regex) {
        panic!("LogNotExists matched {pattern:?} on line: {line}");
    }
    info!(
        "LogNotExists matched no line in {} captured log line(s)",
        lines.len()
    );
}
