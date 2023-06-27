use crate::kernel::process::Priority;

pub const ROOM_SCANNING_PRIORITY: Priority = 230;
pub const ROOM_PLANNING_PRIORITY: Priority = 80;
pub const CONSTRUCTING_STRUCTURES_PRIORITY: Priority = 100;
pub const MINING_PRIORITY: Priority = 180;
pub const CREEP_REGISTRATION_PRIORITY: Priority = 220;
pub const ROOM_MAINTENANCE_PRIORITY: Priority = 200;
pub const SPAWNING_CREEPS_PRIORITY: Priority = 50;
pub const VISUALIZATIONS_PRIORITY: Priority = 10;

pub const MINER_SPAWN_PRIORITY: Priority = 200;