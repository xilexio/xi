use crate::kernel::{fresh_cid, move_current_process_to_waiting_for_condition, signal_condition};
use log::trace;
use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll};

/// Condition Identifier.
pub type Cid = u32;

/// A generic condition to wait on. Can be awaited until `condition.signal(value)` is called.
#[derive(Clone)]
pub struct Condition<T> {
    cid: Cid,
    value: Rc<RefCell<Option<T>>>,
}

impl<T> Condition<T> {
    pub fn new() -> Self {
        Condition {
            cid: fresh_cid(),
            value: Rc::new(RefCell::new(None)),
        }
    }

    pub fn signal(&mut self, value: T) {
        self.value.replace(Some(value));
        signal_condition(self.cid);
    }
}

impl<T> Future for Condition<T>
where
    T: Clone,
{
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.value.borrow().as_ref() {
            None => {
                trace!("Condition pending.");
                move_current_process_to_waiting_for_condition(self.cid);
                Poll::Pending
            }
            Some(x) => {
                trace!("Condition ready.",);
                Poll::Ready(x.clone())
            }
        }
    }
}
