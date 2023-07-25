use log::LevelFilter;

pub const LOG_LEVEL: LevelFilter = LevelFilter::Trace;

pub const FIRST_MEMORY_SAVE_TICK: u32 = 21;
pub const MEMORY_SAVE_INTERVAL: u32 = 7;

/// The number of ticks ahead that the beginning of creep spawning should be planned.
pub const SPAWN_SCHEDULE_TICKS: u32 = 6000;