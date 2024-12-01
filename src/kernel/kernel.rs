use crate::utils::game_tick::game_tick;
use crate::utils::cold::cold;
use crate::utils::multi_map_utils::{MultiMapUtils, OrderedMultiMapUtils};
use crate::{a, local_debug, u};
use log::{error, trace};
use parking_lot::lock_api::MappedMutexGuard;
use parking_lot::{Mutex, MutexGuard, RawMutex};
use rustc_hash::{FxHashMap, FxHashSet};
use screeps::game;
use std::collections::BTreeMap;
use std::future::Future;
use std::task::Poll;
use crate::kernel::condition::CId;
use crate::kernel::process::{PId, Process, WrappedProcessMeta};
use crate::kernel::process_handle::ProcessHandle;
use crate::kernel::runnable::Runnable;
use crate::utils::priority::Priority;

const DEBUG: bool = false;

/// A singleton executor and reactor. To work correctly, only one Kernel may be used at a time and it must be used
/// from one thread.
#[derive(Debug)]
struct Kernel {
    /// Map from priorities to processes.
    active_processes_by_priorities: BTreeMap<Priority, Vec<Box<dyn Runnable>>>,
    /// Processes that are sleeping until the tick in the key.
    sleeping_processes: BTreeMap<u32, Vec<Box<dyn Runnable>>>,
    /// Processes that are awaiting completion of another process with PID in the key.
    awaiting_processes: FxHashMap<PId, Vec<Box<dyn Runnable>>>,
    /// Processes that are waiting on a condition with the CID in the key.
    condition_processes: FxHashMap<CId, Vec<Box<dyn Runnable>>>,
    /// Processes by PID.
    meta_by_pid: FxHashMap<PId, WrappedProcessMeta>,

    current_process_meta: Option<WrappedProcessMeta>,
}

/// Kernel must not be accessed in a parallel fashion.
unsafe impl Send for Kernel {}

/// Kernel must not be accessed in a parallel fashion.
unsafe impl Sync for Kernel {}

impl Kernel {
    fn new() -> Self {
        Kernel {
            active_processes_by_priorities: BTreeMap::default(),
            sleeping_processes: BTreeMap::default(),
            awaiting_processes: FxHashMap::default(),
            condition_processes: FxHashMap::default(),
            meta_by_pid: FxHashMap::default(),

            current_process_meta: None,
        }
    }
}

/// Schedules a future to run asynchronously. It will not run right away, but instead be enqueued.
/// Returns `ProcessHandle` which can be awaited and returns the value returned by the scheduled process.
/// If called outside of a process, the result should be manually dropped using `std::mem::drop`.
pub fn schedule<F, T>(name: &str, priority: Priority, future: F) -> ProcessHandle<T>
where
    F: Future<Output = T> + 'static,
    T: 'static,
{
    let mut kern = kernel();

    let pid = PId::new();
    let parent_pid = kern.current_process_meta.as_ref().map(|meta| meta.borrow().pid);
    let process = Process::new(name.into(), pid, parent_pid, priority, future);

    let result = process.result.clone();

    kern.meta_by_pid.insert(pid, process.meta.clone());

    trace!("Scheduling {}.", process);

    enqueue_process(&mut kern, Box::new(process));

    ProcessHandle::new(pid, result)
}

/// Kills the process. Can be mildly expensive under some circumstances.
/// Only a process that has not finished or returned yet may be killed.
pub fn kill<T>(process_handle: ProcessHandle<T>, result: T) {
    local_debug!("Killing {}.", process_handle.pid);

    process_handle.result.replace(Some(result));

    kill_without_result_or_cleanup(process_handle.pid);

    cleanup_process(process_handle.pid);
}

/// Kills the process with all its children. Can be mildly expensive under some circumstances.
/// Only a process that has not finished or returned yet may be killed.
/// Furthermore, there must not exist any process awaiting completion of the process' children except for the process
/// or its children themselves.
// TODO Processes whose parents are already finished but given process is an ancestors will not be killed.
pub fn kill_tree<T>(process_handle: ProcessHandle<T>, result: T) {
    local_debug!("Killing tree of {}.", process_handle.pid);

    let mut killed_pids = FxHashSet::default();
    let mut awaiting_pids = FxHashSet::default();
    {
        let kern = kernel();

        let mut processes_children = FxHashMap::default();
        for (&pid, meta) in kern.meta_by_pid.iter() {
            if let Some(parent_pid) = meta.borrow().parent_pid {
                processes_children.push_or_insert(parent_pid, pid);
            }
        }

        let mut queue = processes_children.remove(&process_handle.pid).unwrap_or(Vec::new());
        while let Some(pid) = queue.pop() {
            let children = processes_children.remove(&pid).unwrap_or(Vec::new());
            killed_pids.extend(children.iter().cloned());
            queue.extend(children.into_iter());
            if let Some(awaiting_processes) = kern.awaiting_processes.get(&pid) {
                awaiting_pids.extend(awaiting_processes.iter().map(|process| process.borrow_meta().pid));
            }
        }
    }

    for pid in killed_pids {
        local_debug!("Killing {}.", pid);
        awaiting_pids.remove(&pid);
        kill_without_result_or_cleanup(pid);
    }

    awaiting_pids.remove(&process_handle.pid);

    // There should be no process awaiting any killed processes except for the killed ones.
    a!(awaiting_pids.is_empty());

    kill(process_handle, result);
}

fn kill_without_result_or_cleanup(pid: PId) {
    let mut kern = kernel();
    // None indicates the process has finished already.
    if let Some(removed_meta) = kern.meta_by_pid.remove(&pid) {
        local_debug!("Removing meta of process {}.", pid);
        let meta = removed_meta.borrow();
        let process = if let Some(wake_up_tick) = meta.wake_up_tick {
            drop(meta);
            local_debug!("Process {} was awaiting tick {}.", pid, wake_up_tick);
            let vec_with_process = u!(kern.sleeping_processes.get_mut(&wake_up_tick));
            let process = u!(vec_with_process
                .extract_if(|process| process.borrow_meta().pid == pid).next()
            );
            if vec_with_process.is_empty() {
                kern.sleeping_processes.remove(&wake_up_tick);
            }
            process
        } else if let Some(awaited_pid) = meta.awaited_pid {
            drop(meta);
            local_debug!("Process {} was awaiting {}.", pid, awaited_pid);
            let vec_with_process = u!(kern.awaiting_processes.get_mut(&awaited_pid));
            let process = u!(vec_with_process
                .extract_if(|process| process.borrow_meta().pid == pid).next()
            );
            if vec_with_process.is_empty() {
                kern.awaiting_processes.remove(&awaited_pid);
            }
            process
        } else if let Some(awaited_cid) = meta.awaited_cid {
            drop(meta);
            local_debug!("Process {} was awaiting condition {}.", pid, awaited_cid);
            let vec_with_process = u!(kern.condition_processes.get_mut(&awaited_cid));
            let process = u!(vec_with_process
                .extract_if(|process| process.borrow_meta().pid == pid).next()
            );
            if vec_with_process.is_empty() {
                kern.condition_processes.remove(&awaited_cid);
            }
            process
        } else {
            let priority = meta.priority;
            drop(meta);
            local_debug!("Process {} was not awaiting anything.", pid);
            // Fail on unwrap means that the process was neither awaiting anything nor active,
            // which should never happen.
            let vec_with_process = u!(kern.active_processes_by_priorities.get_mut(&priority));
            let process = u!(vec_with_process
                .extract_if(|process| process.borrow_meta().pid == pid).next()
            );
            if vec_with_process.is_empty() {
                kern.active_processes_by_priorities.remove(&priority);
            }
            process
        };

        trace!("Killed {}.", process);
    } else {
        local_debug!("Meta of process {} was already removed.", pid);
    }
}

/// Runs all processes in the queue. Should be preceded by waking up all sleeping processes that should wake up this
/// tick and waking up all processes waiting for travel to finish.
pub fn run_processes() {
    while let Some((_, mut process)) = { (|| kernel().active_processes_by_priorities.pop_from_last())() } {
        trace!("Running {}.", process);

        let pid = process.borrow_meta().pid;

        kernel().current_process_meta = Some(process.clone_meta());

        match process.poll() {
            Poll::Ready(()) => {
                trace!("{} finished.", process);
                cleanup_process(pid);
            }
            Poll::Pending => {
                let mut kern = kernel();
                let meta = u!(kern.current_process_meta.as_ref()).borrow_mut();

                if let Some(awaited_process_pid) = meta.awaited_pid {
                    drop(meta);
                    local_debug!("{} waiting for {}.", process, awaited_process_pid);
                    kern.awaiting_processes.push_or_insert(awaited_process_pid, process);
                } else if let Some(wake_up_tick) = meta.wake_up_tick {
                    drop(meta);
                    local_debug!("{} sleeping until {}.", process, wake_up_tick);
                    kern.sleeping_processes.push_or_insert(wake_up_tick, process);
                } else if let Some(awaited_cid) = meta.awaited_cid {
                    drop(meta);
                    local_debug!("{} waiting for {}.", process, awaited_cid);
                    kern.condition_processes.push_or_insert(awaited_cid, process);
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
    let mut kern = kernel();

    while let Some(first_entry) = kern.sleeping_processes.first_entry() {
        if game_tick() <= *first_entry.key() {
            for process in first_entry.remove() {
                process.borrow_meta().wake_up_tick = None;
                enqueue_process(&mut kern, process);
            }
            continue;
        }
    }
}

pub(super) fn move_current_process_to_awaiting(awaited_process_pid: PId) {
    if let Some(meta) = kernel().current_process_meta.as_ref() {
        meta.borrow_mut().awaited_pid = Some(awaited_process_pid);
    } else {
        error!("Tried await completion of a process while there is no current process.")
    }
}

pub(super) fn move_current_process_to_sleeping(wake_up_tick: u32) {
    if let Some(meta) = kernel().current_process_meta.as_ref() {
        meta.borrow_mut().wake_up_tick = Some(wake_up_tick);
    } else {
        error!("Tried to sleep while there is no current process.");
    }
}

pub(super) fn signal_condition(cid: CId) {
    let mut kern = kernel();

    // Ignoring when a condition is not waited on since it is not required and may happen instantly.
    if let Some(processes) = kern.condition_processes.remove(&cid) {
        for process in processes {
            process.borrow_meta().awaited_cid = None;
            enqueue_process(&mut kern, process);
        }
    }
}

pub(super) fn move_current_process_to_waiting_for_condition(cid: CId) {
    if let Some(meta) = kernel().current_process_meta.as_ref() {
        meta.borrow_mut().awaited_cid = Some(cid);
    } else {
        error!("Tried to wait on a condition while there is no current process.");
    }
}

/// Perform actions made after a process has ended and was removed from one of kernel process collections.
fn cleanup_process(pid: PId) {
    let mut kern = kernel();

    let maybe_awaiting_processes = kern.awaiting_processes.remove(&pid);
    if let Some(awaiting_processes) = maybe_awaiting_processes {
        for awaiting_process in awaiting_processes {
            trace!("Waking up {}.", awaiting_process);
            awaiting_process.borrow_meta().awaited_pid = None;
            enqueue_process(&mut kern, awaiting_process);
        }
    }

    // The meta may be not present in `meta_by_pid` anymore if the process was killed.
    kern.meta_by_pid.remove(&pid);

    // TODO Implement in kill somewhere cleanup of conditions no process is awaiting.
    // let meta_ref = meta.borrow();
    // // If the process was waiting on a condition, we need to remove it from there.
    // if let Some(awaited_cid) = meta_ref.awaited_cid {
    //     drop(meta_ref);
    //     match kern.condition_processes.entry(awaited_cid) {
    //         Entry::Occupied(mut occupied_entry) => {
    //             if occupied_entry.get().len() == 1 {
    //                 occupied_entry.remove();
    //             } else {
    //                 occupied_entry
    //                     .get_mut()
    //                     .retain(|process| process.borrow_meta().pid != pid);
    //             }
    //         }
    //         Entry::Vacant(_) => {
    //             unreachable!();
    //         }
    //     }
    // }
}

fn enqueue_process(kern: &mut MappedMutexGuard<RawMutex, Kernel>, process: Box<dyn Runnable>) {
    let priority = process.borrow_meta().priority;
    kern.active_processes_by_priorities.push_or_insert(priority, process);
}

/// Function to be called to check if the process should finish execution for the tick to fit in its CPU time
/// constraints. Should be called regularly from long-running processes.
pub fn should_finish() -> bool {
    // TODO Make this less naive and based on statistics and process parameters.
    game::cpu::get_used() >= 0.8 * game::cpu::tick_limit()
}

/// Borrows metadata of the currently active process. The borrowed reference must be dropped before the next await.
/// Preferably it should be not stored in a variable.
pub fn current_process_wrapped_meta() -> MappedMutexGuard<'static, RawMutex, WrappedProcessMeta> {
    let kern = kernel();

    if kern.current_process_meta.is_some() {
        MappedMutexGuard::map(kern, |k| k.current_process_meta.as_mut().unwrap())
    } else {
        error!("Tried to borrow process meta while there is no current process.");
        unreachable!()
    }
}

pub fn current_priority() -> Priority {
    current_process_wrapped_meta().borrow().priority
}

#[macro_export]
macro_rules! meta(
    () => (
        $current_process_wrapped_meta().borrow_mut()
    );
);

static KERNEL: Mutex<Option<Kernel>> = Mutex::new(None);

/// Returns a guarded reference to the kernel. It cannot be used in two places at once.
fn kernel() -> MappedMutexGuard<'static, RawMutex, Kernel> {
    let mut maybe_kernel = KERNEL.try_lock().unwrap();
    if maybe_kernel.is_none() {
        cold();
        maybe_kernel.replace(Kernel::new());
    }
    MutexGuard::map(maybe_kernel, |k| k.as_mut().unwrap())
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use crate::utils::game_tick::inc_game_time;
    use crate::logging::init_logging;
    use log::LevelFilter::Trace;
    use std::sync::Mutex;
    use log::debug;
    use crate::kernel::broadcast::Broadcast;
    use crate::kernel::condition::Condition;
    use crate::kernel::kernel::{current_process_wrapped_meta, kill, run_processes, schedule, wake_up_sleeping_processes, Kernel, KERNEL};
    use crate::kernel::sleep::sleep;
    use crate::utils::priority::Priority;

    /// Reinitializes the kernel.
    pub fn reset_kernel() {
        KERNEL.try_lock().unwrap().replace(Kernel::new());
    }

    // A mutex to make sure that all tests are executed one after another since the kernel requires a single thread.
    static TEST_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn test_empty_run() {
        let lock = TEST_MUTEX.lock();

        init_logging(Trace);
        reset_kernel();
        run_processes();
    }

    static TEST_COUNTER: Mutex<Cell<u8>> = Mutex::new(Cell::new(0));

    fn set_test_counter(value: u8) {
        TEST_COUNTER.lock().unwrap().set(value);
        debug!("Set test counter to {}.", value);
    }

    fn add_to_test_counter(value: u8) {
        let tc = TEST_COUNTER.lock().unwrap();
        tc.replace(tc.get() + value);
        debug!("Added {} to test counter, current value is {}.", value, tc.get());
    }

    fn get_test_counter() -> u8 {
        TEST_COUNTER.lock().unwrap().get()
    }

    async fn do_stuff() -> u8 {
        add_to_test_counter(1);
        get_test_counter()
    }

    #[test]
    fn test_basic_run() {
        let lock = TEST_MUTEX.lock();

        set_test_counter(0);
        init_logging(Trace);
        reset_kernel();
        assert_eq!(get_test_counter(), 0);
        schedule("do_stuff", Priority(100), do_stuff());
        assert_eq!(get_test_counter(), 0);
        run_processes();
        assert_eq!(get_test_counter(), 1);
        run_processes();
        assert_eq!(get_test_counter(), 1);
    }

    async fn await_do_stuff() {
        add_to_test_counter(1);
        let result = schedule("do_stuff", Priority(100), do_stuff()).await;
        add_to_test_counter(result);
    }

    #[test]
    fn test_awaiting() {
        let lock = TEST_MUTEX.lock();

        set_test_counter(0);
        init_logging(Trace);
        reset_kernel();
        assert_eq!(get_test_counter(), 0);
        schedule("await_do_stuff", Priority(100), await_do_stuff());
        assert_eq!(get_test_counter(), 0);
        run_processes();
        assert_eq!(get_test_counter(), 4);
    }

    async fn do_stuff_and_sleep_and_stuff() {
        add_to_test_counter(1);
        sleep(2).await;
        add_to_test_counter(1);
    }

    #[test]
    fn test_sleep() {
        let lock = TEST_MUTEX.lock();

        set_test_counter(0);
        init_logging(Trace);
        reset_kernel();
        assert_eq!(get_test_counter(), 0);
        schedule(
            "do_stuff_and_sleep_and_stuff",
            Priority(100),
            do_stuff_and_sleep_and_stuff(),
        );
        assert_eq!(get_test_counter(), 0);
        wake_up_sleeping_processes();
        run_processes();
        assert_eq!(get_test_counter(), 1);
        inc_game_time();
        wake_up_sleeping_processes();
        run_processes();
        assert_eq!(get_test_counter(), 1);
        inc_game_time();
        wake_up_sleeping_processes();
        run_processes();
        assert_eq!(get_test_counter(), 2);
        wake_up_sleeping_processes();
        run_processes();
        assert_eq!(get_test_counter(), 2);
    }

    async fn await_sleeping() {
        add_to_test_counter(1);
        schedule("do_stuff_and_sleep_and_stuff", Priority(100), do_stuff_and_sleep_and_stuff()).await;
        add_to_test_counter(1);
    }

    #[test]
    fn test_chained_awaiting_and_sleep() {
        let lock = TEST_MUTEX.lock();

        set_test_counter(0);
        init_logging(Trace);
        reset_kernel();
        assert_eq!(get_test_counter(), 0);
        schedule("await_sleeping", Priority(50), await_sleeping());
        assert_eq!(get_test_counter(), 0);
        wake_up_sleeping_processes();
        run_processes();
        assert_eq!(get_test_counter(), 2);
        inc_game_time();
        wake_up_sleeping_processes();
        run_processes();
        assert_eq!(get_test_counter(), 2);
        inc_game_time();
        wake_up_sleeping_processes();
        run_processes();
        assert_eq!(get_test_counter(), 4);
        wake_up_sleeping_processes();
        run_processes();
        assert_eq!(get_test_counter(), 4);
    }

    async fn set_one() {
        set_test_counter(1);
    }

    async fn set_two() {
        set_test_counter(2);
    }

    #[test]
    fn test_priorities() {
        let lock = TEST_MUTEX.lock();

        set_test_counter(0);
        init_logging(Trace);
        reset_kernel();
        schedule("set_one", Priority(50), set_one());
        schedule("set_two", Priority(100), set_two());
        run_processes();
        assert_eq!(get_test_counter(), 1);
        schedule("set_one", Priority(100), set_one());
        schedule("set_two", Priority(50), set_two());
        run_processes();
        assert_eq!(get_test_counter(), 2);
    }

    #[test]
    fn test_closure() {
        let three = 3u8;

        let set_three = async move {
            set_test_counter(three);
        };

        let lock = TEST_MUTEX.lock();

        set_test_counter(0);
        init_logging(Trace);
        reset_kernel();
        schedule("set_three", Priority(100), set_three);
        run_processes();
        assert_eq!(get_test_counter(), 3);
    }

    #[test]
    fn test_changing_priority() {
        let set_four = async move {
            set_test_counter(4);
            current_process_wrapped_meta().borrow_mut().priority = Priority(150);
            sleep(1).await;
            set_test_counter(4);
        };

        let set_five = async move {
            set_test_counter(5);
            sleep(1).await;
            set_test_counter(5);
        };

        let lock = TEST_MUTEX.lock();

        set_test_counter(0);
        init_logging(Trace);
        reset_kernel();
        schedule("set_four", Priority(50), set_four);
        schedule("set_five", Priority(100), set_five);
        run_processes();
        assert_eq!(get_test_counter(), 4);
        inc_game_time();
        wake_up_sleeping_processes();
        run_processes();
        assert_eq!(get_test_counter(), 5);
        inc_game_time();
    }

    #[test]
    fn test_kill() {
        let spawn_and_kill = async {
            let process_handle = schedule("increment", Priority(50), async {
                loop {
                    add_to_test_counter(1);
                    sleep(1).await;
                }
            });
            sleep(1).await;
            let ph = process_handle.clone();
            schedule("kill", Priority(100), async {
                kill(ph, 10);
            });
            let result = process_handle.await;
            add_to_test_counter(result);
        };

        let lock = TEST_MUTEX.lock();

        set_test_counter(0);
        init_logging(Trace);
        reset_kernel();
        schedule("spawn_and_kill", Priority(100), spawn_and_kill);
        run_processes();
        assert_eq!(get_test_counter(), 1);
        inc_game_time();
        wake_up_sleeping_processes();
        run_processes();
        assert_eq!(get_test_counter(), 11);
    }

    #[test]
    fn test_two_processes_waiting_for_one() {
        let lock = TEST_MUTEX.lock();

        set_test_counter(0);
        init_logging(Trace);
        reset_kernel();
        schedule("waiting_outer", Priority(100), async {
            let waited = schedule("waited", Priority(99), async {
                add_to_test_counter(1);
                sleep(1).await;
                add_to_test_counter(1);
                42
            });
            let waited_copy = waited.clone();
            schedule("waiting_inner", Priority(98), async {
                sleep(2).await;
                let value = waited_copy.await;
                add_to_test_counter(value);
            });
            add_to_test_counter(waited.await);
        });
        run_processes();
        assert_eq!(get_test_counter(), 1);
        inc_game_time();
        wake_up_sleeping_processes();
        run_processes();
        assert_eq!(get_test_counter(), 44);
        inc_game_time();
        wake_up_sleeping_processes();
        run_processes();
        assert_eq!(get_test_counter(), 86);
    }

    #[test]
    fn test_condition() {
        let lock = TEST_MUTEX.lock();

        set_test_counter(0);
        init_logging(Trace);
        reset_kernel();
        schedule("waker", Priority(100), async {
            let cond = Condition::<u8>::default();
            let cond_copy1 = cond.clone();
            let cond_copy2 = cond.clone();
            schedule("waiter_immediate", Priority(99), async {
                sleep(2).await;
                add_to_test_counter(cond_copy1.await);
            });
            schedule("waiter", Priority(99), async {
                add_to_test_counter(cond_copy2.await);
            });
            sleep(1).await;
            cond.signal(42);
        });
        run_processes();
        assert_eq!(get_test_counter(), 0);
        inc_game_time();
        wake_up_sleeping_processes();
        run_processes();
        assert_eq!(get_test_counter(), 42);
        inc_game_time();
        wake_up_sleeping_processes();
        run_processes();
        assert_eq!(get_test_counter(), 84);
    }

    #[test]
    fn test_broadcast() {
        let lock = TEST_MUTEX.lock();

        set_test_counter(0);
        init_logging(Trace);
        reset_kernel();
        schedule("waker", Priority(100), async {
            let cond = Broadcast::<u8>::default();
            let cond_copy1 = cond.clone_primed();
            let cond_copy2 = cond.clone_primed();
            schedule("waiter_immediate", Priority(99), async move {
                sleep(2).await;
                add_to_test_counter(cond_copy1.await);
            });
            schedule("waiter", Priority(99), async move {
                add_to_test_counter(cond_copy2.await);
            });
            sleep(1).await;
            cond.broadcast(42);
        });
        run_processes();
        assert_eq!(get_test_counter(), 0);
        inc_game_time();
        wake_up_sleeping_processes();
        run_processes();
        assert_eq!(get_test_counter(), 42);
        inc_game_time();
        wake_up_sleeping_processes();
        run_processes();
        assert_eq!(get_test_counter(), 84);
    }

    #[test]
    fn test_broadcast_not_primed() {
        let lock = TEST_MUTEX.lock();

        set_test_counter(0);
        init_logging(Trace);
        reset_kernel();
        schedule("waker", Priority(100), async {
            let cond = Broadcast::<u8>::default();
            let cond_copy1 = cond.clone_primed();
            let cond_copy2 = cond.clone_primed();
            schedule("waiter1", Priority(99), async move {
                sleep(2).await;
                let cond_copy1_copy = cond_copy1.clone_not_primed();
                add_to_test_counter(cond_copy1_copy.await);
            });
            schedule("waiter2", Priority(99), async move {
                let cond_copy2_copy = cond_copy2.clone_not_primed();
                add_to_test_counter(cond_copy2_copy.await);
            });
            sleep(1).await;
            cond.broadcast(1);
            sleep(2).await;
            cond.broadcast(2);
        });
        run_processes();
        assert_eq!(get_test_counter(), 0);
        inc_game_time();
        wake_up_sleeping_processes();
        run_processes();
        assert_eq!(get_test_counter(), 1);
        inc_game_time();
        wake_up_sleeping_processes();
        run_processes();
        assert_eq!(get_test_counter(), 1);
        inc_game_time();
        wake_up_sleeping_processes();
        run_processes();
        assert_eq!(get_test_counter(), 3);
    }

    #[test]
    fn test_broadcast_manual_check() {
        let lock = TEST_MUTEX.lock();

        set_test_counter(0);
        init_logging(Trace);
        reset_kernel();
        schedule("waker", Priority(100), async {
            let cond = Broadcast::<u8>::default();
            let mut cond_copy = cond.clone_primed();
            schedule("checker", Priority(99), async move {
                assert_eq!(cond_copy.check(), None);
                sleep(1).await;
                assert_eq!(cond_copy.check(), Some(1));
                assert_eq!(cond_copy.check(), None);
                sleep(1).await;
                assert_eq!(cond_copy.check(), None);
                sleep(1).await;
                assert_eq!(cond_copy.check(), Some(2));
                assert_eq!(cond_copy.check(), None);
            });
            sleep(1).await;
            cond.broadcast(1);
            sleep(2).await;
            cond.broadcast(2);
        });
        run_processes();
        inc_game_time();
        wake_up_sleeping_processes();
        run_processes();
        inc_game_time();
        wake_up_sleeping_processes();
        run_processes();
        inc_game_time();
        wake_up_sleeping_processes();
        run_processes();
    }

    #[test]
    fn test_broadcast_in_loop() {
        let lock = TEST_MUTEX.lock();

        set_test_counter(0);
        init_logging(Trace);
        reset_kernel();
        schedule("waker", Priority(100), async {
            let cond = Broadcast::<u8>::default();
            let cond_copy = cond.clone_primed();
            schedule("loop", Priority(99), async move {
                for x in 0..3 {
                    add_to_test_counter(cond_copy.clone_not_primed().await);
                }
            });
            sleep(1).await;
            cond.broadcast(1);
            sleep(1).await;
            cond.broadcast(2);
            sleep(1).await;
            cond.broadcast(3);
        });
        run_processes();
        assert_eq!(get_test_counter(), 0);
        inc_game_time();
        wake_up_sleeping_processes();
        run_processes();
        assert_eq!(get_test_counter(), 1);
        inc_game_time();
        wake_up_sleeping_processes();
        run_processes();
        assert_eq!(get_test_counter(), 3);
        inc_game_time();
        wake_up_sleeping_processes();
        run_processes();
        assert_eq!(get_test_counter(), 6);
    }
}