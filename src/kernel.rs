pub mod process;
pub mod sleep;

use crate::kernel::process::{BorrowedProcessMeta, Pid, Priority, ProcessMeta};
use derive_more::Constructor;
use futures::pin_mut;
use log::debug;
use process::Process;
use std::collections::{BinaryHeap, BTreeMap, HashMap};
use std::collections::btree_map::Entry;
use std::future::Future;
use std::mem::MaybeUninit;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use rustc_hash::FxHashMap;
use crate::a;
use crate::kernel::sleep::sleep;

pub struct Kernel {
    processes_by_priorities: BTreeMap<Priority, FxHashMap<Pid, Process>>,
}

impl Kernel {
    pub fn new() -> Self {
        Kernel {
            processes_by_priorities: BTreeMap::default(),
        }
    }

    pub fn schedule<P, F>(&mut self, process_fn: P)
    where
        P: FnMut(BorrowedProcessMeta) -> F,
        F: Future<Output = ()> + 'static,
    {
        let mut process = Process::new(process_fn);

        let waker = Waker::from(Arc::new(KernelWaker::new()));
        let mut cx = Context::from_waker(&waker);

        match process.future.as_mut().poll(&mut cx) {
            Poll::Ready(x) => {
                a!(process.meta.borrow().initialized);
                debug!("Process exited right after initialization.");
            }
            Poll::Pending => {
                let (pid, priority) = {
                    let meta = process.meta.borrow();
                    (meta.pid, meta.priority)
                };
                match self.processes_by_priorities.entry(priority) {
                    Entry::Occupied(mut e) => {
                        e.get_mut().insert(pid, process);
                    }
                    Entry::Vacant(e) => {
                        e.insert([(pid, process)].into_iter().collect());
                    }
                }

            }
        }



        // let meta = process.meta();
        // let priority_map = self.processes_by_priorities.entry(meta.priority).or_insert_with(|| {
        //     self.priorities.push(meta.priority);
        //     HashMap::new()
        // });
        // priority_map.insert(meta.pid, process);
    }

    pub fn run(&mut self) {
        // for (priority, priority_map) in &self.processes_by_priorities {
        //     for (pid, process) in priority_map {
        //         debug!(
        //             "Running process {} with PID {} and priority {}.",
        //             process.name(),
        //             pid,
        //             priority
        //         );
        //         process.run();
        //     }
        // }
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
