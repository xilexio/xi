use crate::kernel::kernel::{move_current_process_to_waiting_for_condition, signal_condition};
use log::trace;
use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll};
use crate::kernel::cid::CId;

/// A generic condition to wait on. Can be awaited until `condition.signal(value)` is called.
#[derive(Debug, Clone)]
pub struct Condition<T> {
    pub cid: CId,
    value: Rc<RefCell<Option<T>>>,
}

impl<T> Default for Condition<T> {
    fn default() -> Self {
        let cid = CId::new();

        Condition {
            cid,
            value: Rc::new(RefCell::new(None)),
        }
    }
}

impl<T> Condition<T>
where
    T: Clone,
{
    /// Wakes up all processes waiting on the condition.
    pub fn signal(&self, value: T) {
        self.value.replace(Some(value));
        signal_condition(self.cid);
    }

    /// Manually checks if the value has been set.
    pub fn check(&self) -> Option<T> {
        self.value.borrow().as_ref().cloned()
    }
}

impl<T> Future for Condition<T>
where
    T: Clone,
{
    type Output = T;

    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        match self.value.borrow().as_ref() {
            None => {
                trace!("Condition pending.");
                move_current_process_to_waiting_for_condition(self.cid);
                Poll::Pending
            }
            Some(x) => {
                trace!("Condition ready.");
                Poll::Ready(x.clone())
            }
        }
    }
}