use crate::kernel::process::{ProcessMeta, WrappedProcessMeta};
use std::cell::{Ref, RefMut};
use std::fmt::Display;
use std::task::Poll;

pub(super) trait Runnable: Display {
    fn borrow_meta(&self) -> Ref<ProcessMeta>;
    fn borrow_mut_meta(&mut self) -> RefMut<ProcessMeta>;
    fn clone_meta(&self) -> WrappedProcessMeta;
    fn poll(&mut self) -> Poll<()>;
}
