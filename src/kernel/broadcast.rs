use std::rc::Rc;
use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use log::trace;
use crate::game_tick::game_tick;
use crate::kernel::cid::CId;
use crate::kernel::{move_current_process_to_waiting_for_condition, signal_condition};

/// A condition which can be repeatedly waited on. Waits even if there is a value present.
#[derive(Debug)]
pub struct Broadcast<T> {
    cid: CId,
    value: Rc<RefCell<Option<(T, u32)>>>,
    last_try_tick: u32,
}

impl<T> Default for Broadcast<T> {
    fn default() -> Self {
        let cid = CId::new();

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

    /// Clone with same primed state.
    pub fn clone_same(&self) -> Self {
        Broadcast {
            cid: self.cid,
            value: self.value.clone(),
            last_try_tick: self.last_try_tick,
        }
    }

    /// Clone primed.
    pub fn clone_primed(&self) -> Self {
        Broadcast {
            cid: self.cid,
            value: self.value.clone(),
            last_try_tick: 0,
        }
    }
    
    /// Clone not primed.
    pub fn clone_not_primed(&self) -> Self {
        Broadcast {
            cid: self.cid,
            value: self.value.clone(),
            last_try_tick: game_tick(),
        }
    }

    pub fn reset(&self) {
        self.value.replace(None);
    }

    /// Checks if the value changed since last try.
    /// Will not detect more than one broadcast per tick.
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

    // TODO Something is unintuitive here, especially when combined with manual checks.
    fn poll(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        let result = match self.value.borrow().as_ref() {
            None => {
                trace!("Broadcast pending (no data).");
                move_current_process_to_waiting_for_condition(self.cid);
                Poll::Pending
            }
            Some((value, tick)) => {
                if self.last_try_tick < *tick {
                    let cloned_value = value.clone();
                    trace!("Broadcast ready.");
                    Poll::Ready(cloned_value)
                } else {
                    trace!("Broadcast pending (old data).");
                    move_current_process_to_waiting_for_condition(self.cid);
                    Poll::Pending
                }
            }
        };
        self.last_try_tick = game_tick();
        result
    }
}