use crate::game_tick::game_tick;
use crate::kernel::{move_current_process_to_waiting_for_condition, signal_condition};
use log::trace;
use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll};

thread_local! {
    static NEXT_CID: RefCell<Cid> = RefCell::new(0);
}

/// Condition Identifier.
pub type Cid = u32;

/// A generic condition to wait on. Can be awaited until `condition.signal(value)` is called.
#[derive(Debug, Clone)]
pub struct Condition<T> {
    pub cid: Cid,
    value: Rc<RefCell<Option<T>>>,
}

impl<T> Default for Condition<T> {
    fn default() -> Self {
        let cid = fresh_cid();

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

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
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

/// A condition which can be repeatedly waited on. Waits even if there is a value present.
#[derive(Debug)]
pub struct Broadcast<T> {
    cid: Cid,
    value: Rc<RefCell<Option<(T, u32)>>>,
    last_try_tick: u32,
}

impl<T> Clone for Broadcast<T> {
    fn clone(&self) -> Self {
        Broadcast {
            cid: self.cid,
            value: self.value.clone(),
            last_try_tick: 0,
        }
    }
}

impl<T> Default for Broadcast<T> {
    fn default() -> Self {
        let cid = fresh_cid();

        Broadcast {
            cid,
            value: Rc::new(RefCell::new(None)),
            last_try_tick: 0,
        }
    }
}

impl<T> Broadcast<T>
where
    T: Clone,
{
    /// Wakes up all processes waiting on the broadcast.
    pub fn broadcast(&self, value: T) {
        self.value.replace(Some((value, game_tick())));
        signal_condition(self.cid);
    }
    
    /// Clone with manual check ignoring anything happening this or previous ticks.
    pub fn clone_inactive(&self) -> Self {
        Broadcast {
            cid: self.cid,
            value: self.value.clone(),
            last_try_tick: game_tick(),
        }
    }

    pub fn reset(&self) {
        self.value.replace(None);
    }

    /// Checks if the value changed since last try. Will not detect more than one broadcast per tick.
    pub fn check(&mut self) -> Option<T> {
        match self.value.borrow().as_ref() {
            None => None,
            Some((value, tick)) => {
                if *tick > self.last_try_tick {
                    self.last_try_tick = *tick;
                    Some(value.clone())
                } else {
                    None
                }
            }
        }
    }
}

impl<T> Future for Broadcast<T>
where
    T: Clone,
{
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.value.borrow().as_ref() {
            None => {
                trace!("Broadcast pending (no data).");
                move_current_process_to_waiting_for_condition(self.cid);
                Poll::Pending
            }
            Some((value, tick)) => {
                if *tick == game_tick() {
                    trace!("Broadcast ready.");
                    Poll::Ready(value.clone())
                } else {
                    trace!("Broadcast pending (old data).");
                    move_current_process_to_waiting_for_condition(self.cid);
                    Poll::Pending
                }
            }
        }
    }
}

fn fresh_cid() -> Cid {
    // Assuming this will never overflow.
    NEXT_CID.with_borrow_mut(|cid| {
        *cid += 1;
        *cid
    })
}
