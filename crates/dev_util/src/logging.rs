//! Logging macros that need no state, only a bevy log target.

/// Emits an info log with the `TEST_LOG` target.
///
/// Scenario `TestLogExists` commands anchor their regexes on this target, e.g.
/// `TestLogExists(regex: "TEST_LOG: .*")`.
#[macro_export]
macro_rules! test_log {
    ($($arg:tt)*) => {
        ::bevy::log::info!(target: "TEST_LOG", $($arg)*)
    };
}

/// Warn only once about something.
#[macro_export]
macro_rules! warn_once {
    ($($arg:tt)*) => {{
        static ONCE: ::std::sync::Once = ::std::sync::Once::new();

        ONCE.call_once(|| {
            ::bevy::log::warn!($($arg)*);
        });
    }};
}

/// Warn only once that something is unimplemented.
#[macro_export]
macro_rules! warn_unimplemented {
    ($($arg:tt)*) => {{
        $crate::warn_once!("unimplemented: {}", format_args!($($arg)*));
    }};
}
