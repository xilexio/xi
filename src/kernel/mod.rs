use crate::kernel::kernel::Kernel;
use crate::unwrap;

pub mod kernel;
pub mod process;
pub mod process_result;
pub mod runnable;
pub mod sleep;

static mut KERNEL: Option<Kernel> = None;

/// Initializes or reinitializes the kernel.
pub fn init_kernel() {
    unsafe { KERNEL = Some(Kernel::new()) }
}

/// Returns a reference to the kernel. Panics if it wasn't initialized.
/// It is undefined behavior to hold onto this reference. The pattern of usage should always be kernel().method().
pub fn kernel() -> &'static mut Kernel {
    unsafe { unwrap!(KERNEL.as_mut()) }
}

#[cfg(test)]
mod tests {
    use crate::game_time::GAME_TIME;
    use crate::kernel::sleep::sleep;
    use crate::kernel::{init_kernel, kernel};
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
        kernel().run();
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
        drop(kernel().schedule("do_stuff", 100, do_stuff));
        unsafe {
            assert_eq!(TEST_COUNTER, 0);
        }
        kernel().run();
        unsafe {
            assert_eq!(TEST_COUNTER, 1);
        }
        kernel().run();
        unsafe {
            assert_eq!(TEST_COUNTER, 1);
        }
    }

    async fn await_do_stuff() {
        unsafe {
            TEST_COUNTER += 1;
        }
        let result = kernel().schedule("do_stuff", 100, do_stuff).await;
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
        drop(kernel().schedule("await_do_stuff", 100, await_do_stuff));
        unsafe {
            assert_eq!(TEST_COUNTER, 0);
        }
        kernel().run();
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
        drop(kernel().schedule("do_stuff_and_sleep_and_stuff", 100, do_stuff_and_sleep_and_stuff));
        unsafe {
            assert_eq!(TEST_COUNTER, 0);
        }
        kernel().wake_up_sleeping();
        kernel().run();
        unsafe {
            assert_eq!(TEST_COUNTER, 1);
            GAME_TIME += 1;
        }
        kernel().wake_up_sleeping();
        kernel().run();
        unsafe {
            assert_eq!(TEST_COUNTER, 1);
            GAME_TIME += 1;
        }
        kernel().wake_up_sleeping();
        kernel().run();
        unsafe {
            assert_eq!(TEST_COUNTER, 2);
        }
        kernel().wake_up_sleeping();
        kernel().run();
        unsafe {
            assert_eq!(TEST_COUNTER, 2);
        }
    }

    async fn await_sleeping() {
        unsafe {
            TEST_COUNTER += 1;
        }
        kernel()
            .schedule("do_stuff_and_sleep_and_stuff", 100, do_stuff_and_sleep_and_stuff)
            .await;
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
        drop(kernel().schedule("await_sleeping", 50, await_sleeping));
        unsafe {
            assert_eq!(TEST_COUNTER, 0);
        }
        kernel().wake_up_sleeping();
        kernel().run();
        unsafe {
            assert_eq!(TEST_COUNTER, 2);
            GAME_TIME += 1;
        }
        kernel().wake_up_sleeping();
        kernel().run();
        unsafe {
            assert_eq!(TEST_COUNTER, 2);
            GAME_TIME += 1;
        }
        kernel().wake_up_sleeping();
        kernel().run();
        unsafe {
            assert_eq!(TEST_COUNTER, 4);
        }
        kernel().wake_up_sleeping();
        kernel().run();
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
        drop(kernel().schedule("set_one", 50, set_one));
        drop(kernel().schedule("set_two", 100, set_two));
        kernel().run();
        unsafe {
            assert_eq!(TEST_COUNTER, 2);
        }
        drop(kernel().schedule("set_one", 100, set_one));
        drop(kernel().schedule("set_two", 50, set_two));
        kernel().run();
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
        drop(kernel().schedule("set_three", 100, set_three));
        kernel().run();
        unsafe {
            assert_eq!(TEST_COUNTER, 3);
        }
    }

    #[test]
    fn test_changing_priority() {
        let set_four = async move || unsafe {
            TEST_COUNTER = 4;
            kernel().borrow_mut_meta().priority = 150;
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
        drop(kernel().schedule("set_four", 50, set_four));
        drop(kernel().schedule("set_five", 100, set_five));
        kernel().run();
        unsafe {
            assert_eq!(TEST_COUNTER, 5);
            GAME_TIME += 1;
        }
        kernel().wake_up_sleeping();
        kernel().run();
        unsafe {
            assert_eq!(TEST_COUNTER, 4);
            GAME_TIME += 1;
        }
    }
}
