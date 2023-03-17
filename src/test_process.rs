use log::info;
use crate::kernel::process::{Process, ProcessMeta};

pub struct TestProcess {
    pub meta: ProcessMeta,
}

impl Process for TestProcess {
    fn meta(&self) -> &ProcessMeta {
        &self.meta
    }

    fn run(&self) {
        info!("Test process running!")
    }

    fn name(&self) -> &str {
        "TestProcess"
    }
}

impl TestProcess {

}