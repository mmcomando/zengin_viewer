//! Logs asserted by scenario `TestLogExists` commands. Lines carry the stable
//! `TEST_LOG` target so scenario regexes can anchor on it, e.g.
//! `TestLogExists(regex: "TEST_LOG: .*")`.

/// Emits an info log with the `TEST_LOG` target.
#[macro_export]
macro_rules! test_log {
    ($($arg:tt)*) => {
        ::bevy::log::info!(target: "TEST_LOG", $($arg)*)
    };
}
