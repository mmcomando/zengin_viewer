#[macro_export]
macro_rules! warn_once {
    ($($arg:tt)*) => {{
        static ONCE: std::sync::Once = std::sync::Once::new();

        ONCE.call_once(|| {
            bevy::log::warn!($($arg)*);
        });
    }};
}

#[macro_export]
macro_rules! gothic_unimplemented {
    ($($arg:tt)*) => {{
        warn_once!("GOTHIC unimplemented: {}", format_args!($($arg)*));
    }};
}
