//! Warn macros used by the loaders and the rest of the crate.

/// Warn only once about something
#[macro_export]
macro_rules! warn_once {
    ($($arg:tt)*) => {{
        static ONCE: std::sync::Once = std::sync::Once::new();

        ONCE.call_once(|| {
            bevy::log::warn!($($arg)*);
        });
    }};
}

/// Warn only once that something is unimplemented
#[macro_export]
macro_rules! warn_unimplemented {
    ($($arg:tt)*) => {{
        $crate::warn_once!("unimplemented: {}", format_args!($($arg)*));
    }};
}
