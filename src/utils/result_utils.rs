use log::warn;
use std::fmt::Debug;

pub trait ResultUtils<T, E>
where
    Self: Debug,
{
    fn warn_if_err(&self, description: &str);
}

impl<T, E> ResultUtils<T, E> for Result<T, E>
where
    T: Debug,
    E: Debug,
{
    fn warn_if_err(&self, description: &str) {
        if self.is_err() {
            warn!("{}: {:?}.", description, self);
        }
    }
}
