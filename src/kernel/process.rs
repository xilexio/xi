use crate::kernel::sleep::{sleep, Sleep};
use derive_more::Constructor;
use futures::pin_mut;
use std::cell::{RefCell, RefMut};
use std::future::Future;
use std::pin::Pin;
use std::process::Output;
use std::rc::Rc;

pub type Pid = u32;
pub type Priority = u8;

pub struct ProcessMeta {
    pub pid: Pid,
    pub initialized: bool,
    pub priority: Priority,
    pub creeps: Vec<String>,
}

pub struct Process {
    pub meta: Rc<RefCell<ProcessMeta>>,
    pub future: Pin<Box<dyn Future<Output = ()>>>,
}

impl Process {
    pub fn new<P, F>(mut process_fn: P) -> Self
    where
        P: FnMut(BorrowedProcessMeta) -> F,
        F: Future<Output = ()> + 'static,
    {
        let mut meta = ProcessMeta {
            pid: 0,
            initialized: false,
            priority: 50,
            creeps: Vec::new(),
        };
        let wrapped_meta = Rc::new(RefCell::new(meta));

        let future = process_fn(BorrowedProcessMeta::new(wrapped_meta.clone()));
        let boxed_future = Box::pin(future);

        Process {
            meta: wrapped_meta,
            future: boxed_future,
        }
    }
}

#[derive(Clone, Constructor)]
pub struct BorrowedProcessMeta {
    meta: Rc<RefCell<ProcessMeta>>,
}

impl BorrowedProcessMeta {
    pub fn with<F, R>(&self, mut f: F) -> R
    where
        F: FnMut(RefMut<'_, ProcessMeta>) -> R,
    {
        f(self.meta.borrow_mut())
    }

    pub fn initialize(&self) -> Sleep {
        {
            self.meta.borrow_mut().initialized = true;
        }
        sleep(0)
    }
}

// TODO
// - required creeps to perform the task with priorities and parts
// - ability to schedule creeps ahead of time with given priority to make sure no death spiral occurs,
//   maybe in the form of "make sure these creeps are always available"
// - info where the creeps need to be, distance, etc. (TravelSpec)
// - children processes and ability to wait for them?
//   or maybe make it more efficient by introducing helper functions like getEnergy(process, creep) that may suspend the process
