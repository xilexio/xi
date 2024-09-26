/// Regular `trace!` wrapped in condition that variable `DEBUG` is true.
#[macro_export]
macro_rules! local_trace (
    ($($arg:tt)+) => (
        if DEBUG { trace!($($arg)+) }
    );
);