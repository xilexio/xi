use derive_more::Constructor;
use log::debug;
use screeps::game;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

#[derive(Debug, Constructor)]
pub struct Sleep {
    wake_up_tick: u32,
}

impl Unpin for Sleep {}

impl Future for Sleep {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if game::time() >= self.wake_up_tick {
            debug!(
                "sleep ready because game_time {} >= {} wake_up_tick",
                game::time(),
                self.wake_up_tick
            );
            Poll::Ready(())
        } else {
            debug!(
                "sleep pending because game_time {} < {} wake_up_tick",
                game::time(),
                self.wake_up_tick
            );
            Poll::Pending
        }
    }
}

pub fn sleep(ticks: u32) -> Sleep {
    Sleep::new(game::time() + ticks)
}
