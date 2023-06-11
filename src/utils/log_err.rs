#[macro_export]
macro_rules! log_err (
    ($e:expr) => (
        match $e {
            Ok(_) => (),
            Err(e) => {
                $crate::utils::cold::cold();
                log::error!("Error at {}:{},{} in {}: {}.", file!(), line!(), column!(), module_path!(), e);
            }
        }
    );
);