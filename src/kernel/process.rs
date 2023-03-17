pub type PID = u32;
pub type Priority = u8;

pub struct ProcessMeta {
    pub pid: PID,
    pub priority: Priority,
}

pub trait Process {
    fn meta(&self) -> &ProcessMeta;
    fn run(&self);
    fn name(&self) -> &str;

    // TODO
    // - required creeps to perform the task with priorities and parts
    // - ability to schedule creeps ahead of time with given priority to make sure no death spiral occurs,
    //   maybe in the form of "make sure these creeps are always available"
    // - info where the creeps need to be, distance, etc. (TravelSpec)
    // - children processes and ability to wait for them?
    //   or maybe make it more efficient by introducing helper functions like getEnergy(process, creep) that may suspend the process
}