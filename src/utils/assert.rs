use log::error;

#[macro_export]
macro_rules! a(
    ($e:expr) => (
        if !($e) {
            $crate::utils::assert::display_assert_error(module_path!(), file!(), line!(), column!());
            unreachable!();
        }
    );
);

pub fn display_assert_error(module_path: &str, file: &str, line_number: u32, column: u32) {
    error!(
        "Assertion failed in {}:{},{} in {}.",
        file, line_number, column, module_path
    );
}
