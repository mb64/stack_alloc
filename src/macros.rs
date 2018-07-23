//! Logging macro for debugging

macro_rules! debug_log {
    ($format:expr, $($arg:tt)*) => (
        #[cfg(feature = "debug_logs")]
        {
            #[allow(unused_unsafe)]
            unsafe { ::libc::printf($format.as_ptr() as *const i8, $($arg)*); }
        }
    );
    ($format:expr) => (
        debug_log!($format, );
    );
}
