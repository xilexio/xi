use crate::fresh_number::fresh_number;
use crate::game_time::game_time;
use crate::kernel::process::{Pid, Priority, Process, ProcessMeta, WrappedProcessMeta};
use crate::kernel::process_result::ProcessResult;
use crate::kernel::runnable::Runnable;
use crate::map_utils::{MultiMapUtils, OrderedMultiMapUtils};
use crate::unwrap;
use log::{error, trace};
use rustc_hash::FxHashMap;
use std::cell::RefMut;
use std::collections::BTreeMap;
use std::future::Future;
use std::task::Poll;

pub mod process;
pub mod process_result;
pub mod runnable;
pub mod sleep;

/// A singleton executor and reactor. To work correctly, only one Kernel may be used at a time and it must be used
/// from one thread.
struct Kernel {
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
    fn new() -> Self {
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
}

/// Schedules a future to run asynchronously. It will not run right away, but instead be enqueued.
/// Returns `ProcessResult` which can be awaited and returns the value returned by the scheduled process.
/// If called outside of a process, the result should be manually dropped using `std::mem::drop`.
#[must_use]
pub fn schedule<P, F, T>(name: &str, priority: Priority, process_fn: P) -> ProcessResult<T>
where
    P: FnMut() -> F,
    F: Future<Output = T> + 'static,
    T: 'static,
{
    let kern = kernel();

    let pid = fresh_number(&kern.meta_by_pid);
    let parent_pid = kern.current_process_meta.as_ref().map(|meta| meta.borrow().pid);
    let process = Process::new(name.into(), pid, parent_pid, priority, process_fn);

    let result = process.result.clone();

    kern.meta_by_pid.insert(pid, process.meta.clone());
    enqueue_process(Box::new(process));

    ProcessResult::new(pid, result)
}

/// Runs all processes in the queue. Should be preceded by waking up all sleeping processes that should wake up this
/// tick and waking up all processes waiting for travel to finish.
pub fn run_processes() {
    while let Some((priority, mut process)) = kernel().active_processes_by_priorities.pop_from_first() {
        trace!("Running {}.", process);

        let pid = process.borrow_meta().pid;

        kernel().current_process_meta = Some(process.clone_meta());

        match process.poll() {
            Poll::Ready(()) => {
                trace!("{} finished.", process);
                let kern = kernel();

                if let Some(awaiting_processes) = kern.awaiting_processes.remove(&pid) {
                    for awaiting_process in awaiting_processes {
                        trace!("Waking up {}.", awaiting_process);
                        let priority = awaiting_process.borrow_meta().priority;
                        enqueue_process(awaiting_process);
                    }
                }
                kern.meta_by_pid.remove(&pid);
            }
            Poll::Pending => {
                let kern = kernel();

                if let Some(awaited_process_pid) = kern.current_process_awaited_process_pid.take() {
                    trace!("{} waiting for P{}.", process, awaited_process_pid);
                    kern.awaiting_processes.push_or_insert(awaited_process_pid, process);
                } else if let Some(wake_up_tick) = kern.current_process_wake_up_tick.take() {
                    trace!("{} sleeping until {}.", process, wake_up_tick);
                    kern.sleeping_processes.push_or_insert(wake_up_tick, process);
                } else {
                    error!("{} is pending but not waiting for anything.", process)
                }
            }
        }

        kernel().current_process_meta = None;
    }
}

/// Wakes up all sleeping threads if the game tick they were waiting for has come.
pub fn wake_up_sleeping_processes() {
    let kern = kernel();

    while let Some(first_entry) = kern.sleeping_processes.first_entry() {
        if game_time() <= *first_entry.key() {
            for process in first_entry.remove() {
                enqueue_process(process);
            }
        }
    }
}

pub(super) fn move_current_process_to_awaiting(awaited_process_pid: Pid) {
    let kern = kernel();

    if kern.current_process_meta.is_some() {
        kern.current_process_awaited_process_pid = Some(awaited_process_pid);
    } else {
        error!("Tried await completion of a process while there is no current process.")
    }
}

pub(super) fn move_current_process_to_sleeping(wake_up_tick: u32) {
    let kern = kernel();

    if kern.current_process_meta.is_some() {
        kern.current_process_wake_up_tick = Some(wake_up_tick);
    } else {
        error!("Tried to sleep while there is no current process.");
    }
}

fn enqueue_process(process: Box<dyn Runnable>) {
    let priority = process.borrow_meta().priority;
    kernel()
        .active_processes_by_priorities
        .push_or_insert(priority, process);
}

/// Borrows metadata of the currently active process. The borrowed reference must be dropped before the next await.
/// Preferably it should be not stored in a variable.
pub fn current_process_mut_meta() -> RefMut<'static, ProcessMeta> {
    if let Some(current_process_meta) = kernel().current_process_meta.as_ref() {
        current_process_meta.borrow_mut()
    } else {
        error!("Tried to borrow process meta while there is no current process.");
        unreachable!()
    }
}

static mut KERNEL: Option<Kernel> = None;

/// Initializes or reinitializes the kernel.
pub fn init_kernel() {
    unsafe { KERNEL = Some(Kernel::new()) }
}

/// Returns a reference to the kernel. Panics if it wasn't initialized.
fn kernel() -> &'static mut Kernel {
    unsafe { unwrap!(KERNEL.as_mut()) }
}

#[cfg(test)]
mod tests {
    use crate::game_time::GAME_TIME;
    use crate::kernel::sleep::sleep;
    use crate::kernel::{current_process_mut_meta, init_kernel, run_processes, schedule, wake_up_sleeping_processes};
    use crate::logging::init_logging;
    use log::LevelFilter::Trace;
    use std::sync::Mutex;

    // A mutex to make sure that all tests are executed one after another since the kernel requires a single thread.
    static mut TEST_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn test_empty_run() {
        let lock = unsafe { TEST_MUTEX.lock() };

        init_logging(Trace);
        init_kernel();
        run_processes();
    }

    static mut TEST_COUNTER: u8 = 0;

    async fn do_stuff() -> u8 {
        unsafe {
            TEST_COUNTER += 1;
            TEST_COUNTER
        }
    }

    #[test]
    fn test_basic_run() {
        let lock = unsafe { TEST_MUTEX.lock() };

        unsafe {
            TEST_COUNTER = 0;
        }
        init_logging(Trace);
        init_kernel();
        unsafe {
            assert_eq!(TEST_COUNTER, 0);
        }
        drop(schedule("do_stuff", 100, do_stuff));
        unsafe {
            assert_eq!(TEST_COUNTER, 0);
        }
        run_processes();
        unsafe {
            assert_eq!(TEST_COUNTER, 1);
        }
        run_processes();
        unsafe {
            assert_eq!(TEST_COUNTER, 1);
        }
    }

    async fn await_do_stuff() {
        unsafe {
            TEST_COUNTER += 1;
        }
        let result = schedule("do_stuff", 100, do_stuff).await;
        unsafe {
            TEST_COUNTER += result;
        }
    }

    #[test]
    fn test_awaiting() {
        let lock = unsafe { TEST_MUTEX.lock() };

        unsafe {
            TEST_COUNTER = 0;
        }
        init_logging(Trace);
        init_kernel();
        unsafe {
            assert_eq!(TEST_COUNTER, 0);
        }
        drop(schedule("await_do_stuff", 100, await_do_stuff));
        unsafe {
            assert_eq!(TEST_COUNTER, 0);
        }
        run_processes();
        unsafe {
            assert_eq!(TEST_COUNTER, 4);
        }
    }

    async fn do_stuff_and_sleep_and_stuff() {
        unsafe {
            TEST_COUNTER += 1;
        }
        sleep(2).await;
        unsafe {
            TEST_COUNTER += 1;
        }
    }

    #[test]
    fn test_sleep() {
        let lock = unsafe { TEST_MUTEX.lock() };

        unsafe {
            TEST_COUNTER = 0;
        }
        init_logging(Trace);
        init_kernel();
        unsafe {
            assert_eq!(TEST_COUNTER, 0);
        }
        drop(schedule(
            "do_stuff_and_sleep_and_stuff",
            100,
            do_stuff_and_sleep_and_stuff,
        ));
        unsafe {
            assert_eq!(TEST_COUNTER, 0);
        }
        wake_up_sleeping_processes();
        run_processes();
        unsafe {
            assert_eq!(TEST_COUNTER, 1);
            GAME_TIME += 1;
        }
        wake_up_sleeping_processes();
        run_processes();
        unsafe {
            assert_eq!(TEST_COUNTER, 1);
            GAME_TIME += 1;
        }
        wake_up_sleeping_processes();
        run_processes();
        unsafe {
            assert_eq!(TEST_COUNTER, 2);
        }
        wake_up_sleeping_processes();
        run_processes();
        unsafe {
            assert_eq!(TEST_COUNTER, 2);
        }
    }

    async fn await_sleeping() {
        unsafe {
            TEST_COUNTER += 1;
        }
        schedule("do_stuff_and_sleep_and_stuff", 100, do_stuff_and_sleep_and_stuff).await;
        unsafe {
            TEST_COUNTER += 1;
        }
    }

    #[test]
    fn test_chained_awaiting_and_sleep() {
        let lock = unsafe { TEST_MUTEX.lock() };

        unsafe {
            TEST_COUNTER = 0;
        }
        init_logging(Trace);
        init_kernel();
        unsafe {
            assert_eq!(TEST_COUNTER, 0);
        }
        drop(schedule("await_sleeping", 50, await_sleeping));
        unsafe {
            assert_eq!(TEST_COUNTER, 0);
        }
        wake_up_sleeping_processes();
        run_processes();
        unsafe {
            assert_eq!(TEST_COUNTER, 2);
            GAME_TIME += 1;
        }
        wake_up_sleeping_processes();
        run_processes();
        unsafe {
            assert_eq!(TEST_COUNTER, 2);
            GAME_TIME += 1;
        }
        wake_up_sleeping_processes();
        run_processes();
        unsafe {
            assert_eq!(TEST_COUNTER, 4);
        }
        wake_up_sleeping_processes();
        run_processes();
        unsafe {
            assert_eq!(TEST_COUNTER, 4);
        }
    }

    async fn set_one() {
        unsafe {
            TEST_COUNTER = 1;
        }
    }

    async fn set_two() {
        unsafe {
            TEST_COUNTER = 2;
        }
    }

    #[test]
    fn test_priorities() {
        let lock = unsafe { TEST_MUTEX.lock() };

        unsafe {
            TEST_COUNTER = 0;
        }
        init_logging(Trace);
        init_kernel();
        drop(schedule("set_one", 50, set_one));
        drop(schedule("set_two", 100, set_two));
        run_processes();
        unsafe {
            assert_eq!(TEST_COUNTER, 2);
        }
        drop(schedule("set_one", 100, set_one));
        drop(schedule("set_two", 50, set_two));
        run_processes();
        unsafe {
            assert_eq!(TEST_COUNTER, 1);
        }
    }

    #[test]
    fn test_closure() {
        let three = 3u8;

        let set_three = async move || unsafe {
            TEST_COUNTER = three;
        };

        let lock = unsafe { TEST_MUTEX.lock() };

        unsafe {
            TEST_COUNTER = 0;
        }
        init_logging(Trace);
        init_kernel();
        drop(schedule("set_three", 100, set_three));
        run_processes();
        unsafe {
            assert_eq!(TEST_COUNTER, 3);
        }
    }

    #[test]
    fn test_changing_priority() {
        let set_four = async move || unsafe {
            TEST_COUNTER = 4;
            current_process_mut_meta().priority = 150;
            sleep(1).await;
            TEST_COUNTER = 4;
        };

        let set_five = async move || unsafe {
            TEST_COUNTER = 5;
            sleep(1).await;
            TEST_COUNTER = 5;
        };

        let lock = unsafe { TEST_MUTEX.lock() };

        unsafe {
            TEST_COUNTER = 0;
        }
        init_logging(Trace);
        init_kernel();
        drop(schedule("set_four", 50, set_four));
        drop(schedule("set_five", 100, set_five));
        run_processes();
        unsafe {
            assert_eq!(TEST_COUNTER, 5);
            GAME_TIME += 1;
        }
        wake_up_sleeping_processes();
        run_processes();
        unsafe {
            assert_eq!(TEST_COUNTER, 4);
            GAME_TIME += 1;
        }
    }
}
