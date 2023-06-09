use crate::a;
use crate::fresh_number::fresh_number;
use crate::game_time::game_time;
use crate::kernel::process::{Pid, Priority, Process, ProcessMeta, WrappedProcessMeta};
use crate::kernel::process_result::ProcessResult;
use crate::kernel::runnable::Runnable;
use crate::map_utils::{MultiMapUtils, OrderedMultiMapUtils};
use log::{error, trace};
use rustc_hash::FxHashMap;
use std::cell::RefMut;
use std::collections::BTreeMap;
use std::future::Future;
use std::task::Poll;

/// A singleton executor and reactor. To work correctly, only one Kernel may be used at a time and it must be used
/// from one thread.
pub struct Kernel {
    /// Map from priorities to processes.
    active_processes_by_priorities: BTreeMap<Priority, Vec<Box<dyn Runnable>>>,
    /// Processes that are sleeping until the tick in the key.
    sleeping_processes: BTreeMap<u32, Vec<Box<dyn Runnable>>>,
    /// Processes that are awaiting completion of another process with PID given in the key.
    awaiting_processes: FxHashMap<Pid, Vec<Box<dyn Runnable>>>,
    /// Processes by PID.
    meta_by_pid: FxHashMap<Pid, WrappedProcessMeta>,

    current_process_meta: Option<WrappedProcessMeta>,
    current_process_wake_up_tick: Option<u32>,
    current_process_awaited_process_pid: Option<Pid>,
}

impl Kernel {
    pub fn new() -> Self {
        Kernel {
            active_processes_by_priorities: BTreeMap::default(),
            sleeping_processes: BTreeMap::default(),
            awaiting_processes: FxHashMap::default(),
            meta_by_pid: FxHashMap::default(),

            current_process_meta: None,
            current_process_wake_up_tick: None,
            current_process_awaited_process_pid: None,
        }
    }

    /// Schedules a future to run asynchronously. It will not run right away, but instead be enqueued.
    /// Returns `ProcessResult` which can be awaited and returns the value returned by the scheduled process.
    /// If called outside of a process, the result should be manually dropped using `std::mem::drop`.
    #[must_use]
    pub fn schedule<P, F, T>(&mut self, name: &str, priority: Priority, process_fn: P) -> ProcessResult<T>
    where
        P: FnMut() -> F,
        F: Future<Output = T> + 'static,
        T: 'static,
    {
        let pid = fresh_number(&self.meta_by_pid);
        let parent_pid = self.current_process_meta.as_ref().map(|meta| meta.borrow().pid);
        let process = Process::new(name.into(), pid, parent_pid, priority, process_fn);

        let result = process.result.clone();

        self.meta_by_pid.insert(pid, process.meta.clone());
        self.enqueue_process(Box::new(process));

        ProcessResult::new(pid, result)
    }

    /// Runs all processes in the queue. Should be preceded by waking up all sleeping processes that should wake up this
    /// tick and waking up all processes waiting for travel to finish.
    pub fn run(&mut self) {
        while let Some((priority, mut process)) = self.active_processes_by_priorities.pop_from_first() {
            trace!("Running {}.", process);

            let pid = process.borrow_meta().pid;

            self.current_process_meta = Some(process.clone_meta());

            match process.poll() {
                Poll::Ready(()) => {
                    trace!("{} finished.", process);
                    if let Some(awaiting_processes) = self.awaiting_processes.remove(&pid) {
                        for awaiting_process in awaiting_processes {
                            trace!("Waking up {}.", awaiting_process);
                            let priority = awaiting_process.borrow_meta().priority;
                            self.enqueue_process(awaiting_process);
                        }
                    }
                    self.meta_by_pid.remove(&pid);
                }
                Poll::Pending => {
                    if let Some(awaited_process_pid) = self.current_process_awaited_process_pid.take() {
                        trace!("{} waiting for P{}.", process, awaited_process_pid);
                        self.awaiting_processes.push_or_insert(awaited_process_pid, process);
                    } else if let Some(wake_up_tick) = self.current_process_wake_up_tick.take() {
                        trace!("{} sleeping until {}.", process, wake_up_tick);
                        self.sleeping_processes.push_or_insert(wake_up_tick, process);
                    } else {
                        error!("{} is pending but not waiting for anything.", process)
                    }
                }
            }

            self.current_process_meta = None;
        }
    }

    pub(super) fn move_current_to_awaiting(&mut self, awaited_process_pid: Pid) {
        if self.current_process_meta.is_some() {
            self.current_process_awaited_process_pid = Some(awaited_process_pid);
        } else {
            error!("Tried await completion of a process while there is no current process.")
        }
    }

    pub(super) fn move_current_to_sleeping(&mut self, wake_up_tick: u32) {
        if self.current_process_meta.is_some() {
            self.current_process_wake_up_tick = Some(wake_up_tick);
        } else {
            error!("Tried to sleep while there is no current process.");
        }
    }

    /// Wakes up all sleeping threads if the game tick they were waiting for has come.
    pub fn wake_up_sleeping(&mut self) {
        while let Some(first_entry) = self.sleeping_processes.first_entry() {
            if game_time() <= *first_entry.key() {
                for process in first_entry.remove() {
                    self.enqueue_process(process);
                }
            }
        }
    }

    fn enqueue_process(&mut self, process: Box<dyn Runnable>) {
        let priority = process.borrow_meta().priority;
        self.active_processes_by_priorities.push_or_insert(priority, process);
    }

    /// Borrows metadata of the currently active process. The borrowed reference must be dropped before the next await.
    /// Preferably it should be not stored in a variable.
    pub fn borrow_mut_meta(&mut self) -> RefMut<ProcessMeta> {
        if let Some(current_process_meta) = self.current_process_meta.as_ref() {
            current_process_meta.borrow_mut()
        } else {
            error!("Tried to borrow process meta while there is no current process.");
            unreachable!()
        }
    }
}
