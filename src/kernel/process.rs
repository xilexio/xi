use crate::kernel::runnable::Runnable;
use derive_more::Constructor;
use std::cell::{RefCell, RefMut};
use std::fmt::{Display, Formatter};
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use crate::kernel::condition::Cid;

pub type Pid = u32;
pub type Priority = u8;

/// Metadata of the process and resources reserved by it.
pub struct ProcessMeta {
    pub name: String,
    pub pid: Pid,
    pub parent_pid: Option<Pid>,
    pub priority: Priority,
    pub creeps: Vec<String>,
    pub wake_up_tick: Option<u32>,
    pub awaited_pid: Option<Pid>,
    pub awaited_cid: Option<Cid>,
}

pub type WrappedProcessMeta = Rc<RefCell<ProcessMeta>>;

pub(super) struct Process<T> {
    pub meta: WrappedProcessMeta,
    pub result: Rc<RefCell<Option<T>>>,
    pub future: Pin<Box<dyn Future<Output = T>>>,
}

impl<T> Process<T> {
    pub(super) fn new<F>(
        name: String,
        pid: Pid,
        parent_pid: Option<Pid>,
        priority: Priority,
        future: F,
    ) -> Self
    where
        F: Future<Output = T> + 'static,
    {
        let meta = ProcessMeta {
            name,
            pid,
            parent_pid,
            priority,
            creeps: Vec::new(),
            wake_up_tick: None,
            awaited_pid: None,
            awaited_cid: None,
        };
        let wrapped_meta = Rc::new(RefCell::new(meta));

        let boxed_future = Box::pin(future);

        Process {
            meta: wrapped_meta,
            result: Rc::new(RefCell::new(None)),
            future: boxed_future,
        }
    }
}

impl<T> Display for Process<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let meta = self.meta.borrow();
        if let Some(parent_pid) = meta.parent_pid {
            write!(f, "P{}-{} ({}/P{})", meta.pid, meta.name, meta.priority, parent_pid)
        } else {
            write!(f, "P{}-{} ({})", meta.pid, meta.name, meta.priority)
        }
    }
}

impl<T> Runnable for Process<T> {
    fn borrow_meta(&self) -> RefMut<ProcessMeta> {
        self.meta.borrow_mut()
    }

    fn clone_meta(&self) -> WrappedProcessMeta {
        self.meta.clone()
    }

    fn poll(&mut self) -> Poll<()> {
        let wake = Arc::new(ProcessWaker::new());
        let waker = Waker::from(wake);
        let mut cx = Context::from_waker(&waker);

        match self.future.as_mut().poll(&mut cx) {
            Poll::Ready(result) => {
                self.result.replace(Some(result));
                Poll::Ready(())
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

/// A no-op waker.
#[derive(Constructor)]
struct ProcessWaker;

impl Wake for ProcessWaker {
    fn wake(self: Arc<Self>) {}

    fn wake_by_ref(self: &Arc<Self>) {}
}

// TODO
// - required creeps to perform the task with priorities and parts
// - ability to schedule creeps ahead of time with given priority to make sure no death spiral occurs,
//   maybe in the form of "make sure these creeps are always available"
// - info where the creeps need to be, distance, etc. (TravelSpec)
// - children processes and ability to wait for them?
//   or maybe make it more efficient by introducing helper functions like getEnergy(process, creep) that may suspend the process
