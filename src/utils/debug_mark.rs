use log::debug;


/// Prints information that this code was ran along with the file and line number.
/// Used for debugging.
#[macro_export]
macro_rules! debug_mark(
    () => (
        $crate::utils::debug_mark::display_debug_mark(module_path!(), file!(), line!(), column!());
    );
);

#[deprecated(note="debug mark should not be present in the finished code")]
pub fn display_debug_mark(module_path: &str, file: &str, line_number: u32, column: u32) {
    debug!(
        "Debug mark in {}:{},{} in {}.",
        file, line_number, column, module_path
    );
}
