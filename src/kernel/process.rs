use crate::kernel::runnable::Runnable;
use derive_more::Constructor;
use std::cell::{RefCell, RefMut};
use std::fmt::{Display, Formatter};
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use crate::kernel::condition::CId;
use crate::utils::priority::Priority;
use crate::utils::uid::UId;

pub type PId = UId<'P'>;

/// Metadata of the process and resources reserved by it.
#[derive(Debug)]
pub struct ProcessMeta {
    pub name: String,
    pub pid: PId,
    pub parent_pid: Option<PId>,
    pub priority: Priority,
    pub creeps: Vec<String>,
    pub wake_up_tick: Option<u32>,
    pub awaited_pid: Option<PId>,
    pub awaited_cid: Option<CId>,
}

impl Display for ProcessMeta {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if let Some(parent_pid) = self.parent_pid {
            write!(f, "{}-{} ({}/{})", self.pid, self.name, self.priority, parent_pid)
        } else {
            write!(f, "{}-{} ({})", self.pid, self.name, self.priority)
        }
    }
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
        pid: PId,
        parent_pid: Option<PId>,
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
        write!(f, "{}", meta)
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