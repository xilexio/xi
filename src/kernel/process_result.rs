use crate::kernel::kernel;
use crate::kernel::process::Pid;
use derive_more::Constructor;
use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll};

/// A structure containing result of a finished process or None before that.
/// It can be awaited and returns the result returned by the process.
#[derive(Clone, Constructor)]
pub struct ProcessResult<T> {
    pid: Pid,
    result: Rc<RefCell<Option<T>>>,
}

impl<T> Future for ProcessResult<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(result) = self.result.borrow_mut().take() {
            Poll::Ready(result)
        } else {
            kernel().move_current_to_awaiting(self.pid);
            Poll::Pending
        }
    }
}
