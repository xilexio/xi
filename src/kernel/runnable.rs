use crate::kernel::process::{ProcessMeta, WrappedProcessMeta};
use std::cell::RefMut;
use std::fmt::{Debug, Display, Formatter};
use std::task::Poll;

pub(super) trait Runnable: Display {
    fn borrow_meta(&self) -> RefMut<ProcessMeta>;

    fn clone_meta(&self) -> WrappedProcessMeta;

    fn poll(&mut self) -> Poll<()>;
}

impl Debug for dyn Runnable {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "<Runnable {}>", self.borrow_meta())
    }
}