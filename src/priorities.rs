use crate::kernel::process::Priority;

// TODO This needs cleanup as order in which processes are executed does not have to be the same as
//      the order in which they get allocated CPU. Also, many of the processes are required for the
//      bot to function at all. 
pub const ROOM_SCANNING_PRIORITY: Priority = 230;
pub const ROOM_PLANNING_PRIORITY: Priority = 80;
pub const CLEANUP_CREEPS_PRIORITY: Priority = 220;
pub const CONSTRUCTING_STRUCTURES_PRIORITY: Priority = 100;
pub const MINING_PRIORITY: Priority = 180;
pub const HAULING_PRIORITY: Priority = 190;
pub const CREEP_REGISTRATION_PRIORITY: Priority = 220;
pub const ROOM_MAINTENANCE_PRIORITY: Priority = 200;
pub const MOVE_CREEPS_PRIORITY: Priority = 50;
pub const SPAWNING_CREEPS_PRIORITY: Priority = 40;
pub const VISUALIZATIONS_PRIORITY: Priority = 10;

pub const MINER_SPAWN_PRIORITY: Priority = 200;
pub const HAULER_SPAWN_PRIORITY: Priority = 150;