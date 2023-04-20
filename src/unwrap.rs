use std::fmt::Debug;
use log::debug;
#[macro_export]
macro_rules! unwrap(
    ($e:expr) => (
        $crate::unwrap::CustomUnwrap::custom_unwrap($e, module_path!(), file!(), line!(), column!())
    );
);

pub trait CustomUnwrap {
    type Type;
    fn custom_unwrap(self, module_path: &str, file: &str, line_number: u32, column: u32) -> Self::Type;
}

impl<T, E: Debug> CustomUnwrap for Result<T, E> {
    type Type = T;

    fn custom_unwrap(self, module_path: &str, file: &str, line_number: u32, column: u32) -> T {
        match self {
            Ok(x) => x,
            Err(e) => {
                debug!(
                    "Unwrapping failed on Result::Err at {}:{},{} in {}: {:?}.",
                    file,
                    line_number,
                    column,
                    module_path,
                    Err::<(), E>(e),
                );
                unreachable!();
            }
        }
    }
}

impl<T> CustomUnwrap for Option<T> {
    type Type = T;

    fn custom_unwrap(self, module_path: &str, file: &str, line_number: u32, column: u32) -> T {
        match self {
            Some(x) => x,
            None => {
                debug!(
                    "Unwrapping failed on Option::None at {}:{},{} in {}.",
                    file, line_number, column, module_path,
                );
                unreachable!();
            }
        }
    }
}
