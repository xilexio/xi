/// Regular `debug!` wrapped in condition that variable `DEBUG` is true.
#[macro_export]
macro_rules! local_debug (
    ($($arg:tt)+) => (
        if DEBUG {
            use log::debug;
            debug!($($arg)+)
        }
    );
);