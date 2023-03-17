pub mod process;

use std::collections::{BinaryHeap, HashMap};
use std::mem::MaybeUninit;
use log::debug;
use process::Process;
use crate::kernel::process::{PID, Priority};

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
        let priority_map = self.processes_by_priorities
            .entry(meta.priority)
            .or_insert_with(|| {
                self.priorities.push(meta.priority);
                HashMap::new()
            });
        priority_map.insert(meta.pid, process);
    }

    pub fn run(&mut self) {
        for (priority, priority_map) in &self.processes_by_priorities {
            for (pid, process) in priority_map {
                debug!("Running process {} with PID {} and priority {}.", process.name(), pid, priority);
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
    unsafe {
        KERNEL.assume_init_mut()
    }
}