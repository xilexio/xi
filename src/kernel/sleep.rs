use crate::game_tick::game_tick;
use crate::kernel::kernel::move_current_process_to_sleeping;
use derive_more::Constructor;
use log::trace;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use crate::local_trace;

const DEBUG: bool = false;

#[derive(Debug, Constructor)]
pub struct Sleep {
    wake_up_tick: u32,
}

impl Future for Sleep {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if game_tick() >= self.wake_up_tick {
            local_trace!(
                "Sleep ready because game_time {} >= {} wake_up_tick.",
                game_tick(),
                self.wake_up_tick
            );
            Poll::Ready(())
        } else {
            local_trace!(
                "Sleep pending because game_time {} < {} wake_up_tick.",
                game_tick(),
                self.wake_up_tick
            );
            move_current_process_to_sleeping(self.wake_up_tick);
            Poll::Pending
        }
    }
}

/// Suspends the current process until given tick.
#[must_use]
pub fn sleep_until(tick: u32) -> Sleep {
    Sleep::new(tick)
}

/// Suspends the current process for given number of ticks.
#[must_use]
pub fn sleep(ticks: u32) -> Sleep {
    Sleep::new(game_tick() + ticks)
}