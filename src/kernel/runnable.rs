use crate::kernel::process::{ProcessMeta, WrappedProcessMeta};
use std::cell::RefMut;
use std::fmt::Display;
use std::task::Poll;

pub(super) trait Runnable: Display {
    fn borrow_meta(&self) -> RefMut<ProcessMeta>;

    fn clone_meta(&self) -> WrappedProcessMeta;

    fn poll(&mut self) -> Poll<()>;
}
