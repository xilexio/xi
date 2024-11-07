use crate::kernel::move_current_process_to_awaiting;
use crate::kernel::process::PId;
use derive_more::Constructor;
use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll};

/// A structure containing result of a finished process or None before that.
/// It can be awaited and returns the result returned by the process.
#[derive(Clone, Debug, Constructor)]
pub struct ProcessHandle<T> {
    pub pid: PId,
    pub(super) result: Rc<RefCell<Option<T>>>,
}

impl<T> Future for ProcessHandle<T>
where
    T: Clone,
{
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(result) = self.result.borrow().as_ref() {
            Poll::Ready(result.clone())
        } else {
            move_current_process_to_awaiting(self.pid);
            Poll::Pending
        }
    }
}
