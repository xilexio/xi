pub mod process;

use crate::kernel::process::{Priority, PID};
use derive_more::Constructor;
use futures::pin_mut;
use log::debug;
use process::Process;
use screeps::game;
use std::collections::{BinaryHeap, HashMap};
use std::future::Future;
use std::mem::MaybeUninit;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};

pub struct Kernel {
    priorities: BinaryHeap<Priority>,
    processes_by_priorities: HashMap<Priority, HashMap<PID, Box<dyn Process>>>,
}

impl Kernel {
    pub fn new() -> Self {
        Kernel {
            priorities: BinaryHeap::new(),
            processes_by_priorities: HashMap::new(),
        }
    }

    pub fn schedule(&mut self, process: Box<dyn Process>) {
        let meta = process.meta();
        let priority_map = self.processes_by_priorities.entry(meta.priority).or_insert_with(|| {
            self.priorities.push(meta.priority);
            HashMap::new()
        });
        priority_map.insert(meta.pid, process);
    }

    pub fn run(&mut self) {
        for (priority, priority_map) in &self.processes_by_priorities {
            for (pid, process) in priority_map {
                debug!(
                    "Running process {} with PID {} and priority {}.",
                    process.name(),
                    pid,
                    priority
                );
                process.run();
            }
        }
    }
}

static mut KERNEL: MaybeUninit<Kernel> = MaybeUninit::uninit();

pub fn init_kernel() {
    unsafe {
        KERNEL.write(Kernel::new());
    }
}

pub fn kernel() -> &'static mut Kernel {
    unsafe { KERNEL.assume_init_mut() }
}

static mut game_time: u32 = 0;

#[derive(Debug, Constructor)]
struct KernelWaker;

impl Wake for KernelWaker {
    fn wake(self: Arc<Self>) {}
}

#[derive(Debug, Constructor)]
pub struct Sleep {
    wake_up_tick: u32,
}

impl Unpin for Sleep {}

impl Future for Sleep {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if unsafe { game_time } >= self.wake_up_tick {
            debug!(
                "sleep ready because game_time {} >= {} wake_up_tick",
                unsafe { game_time },
                self.wake_up_tick
            );
            Poll::Ready(())
        } else {
            debug!(
                "sleep pending because game_time {} < {} wake_up_tick",
                unsafe { game_time },
                self.wake_up_tick
            );
            Poll::Pending
        }
    }
}

pub fn sleep(ticks: u32) -> Sleep {
    Sleep::new(unsafe { game_time } + ticks)
}

pub fn experiment() {
    async fn bar() -> u8 {
        debug!("bar sleep");
        sleep(1).await;
        debug!("bar after sleep");
        42
    }

    async fn foo() {
        debug!("foo");
        let v = bar().await;
        debug!("foo {}", v);
    }

    let f = foo();

    let waker = Waker::from(Arc::new(KernelWaker::new()));
    let mut cx = Context::from_waker(&waker);

    pin_mut!(f);

    for i in 0..2 {
        debug!("game_time {}", unsafe { game_time });

        match f.as_mut().poll(&mut cx) {
            Poll::Ready(x) => {
                debug!("Poll::Ready");
            }
            Poll::Pending => {
                debug!("Poll::Pending");
            }
        }

        unsafe {
            game_time += 1;
        }
    }
}
