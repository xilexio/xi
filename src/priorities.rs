use crate::utils::priority::Priority;

// TODO This needs cleanup as order in which processes are executed does not have to be the same as
//      the order in which they get allocated CPU. Also, many of the processes are required for the
//      bot to function at all. 
pub const ROOM_SCANNING_PRIORITY: Priority = Priority(230);
pub const ROOM_PLANNING_PRIORITY: Priority = Priority(80);
pub const CLEANUP_CREEPS_PRIORITY: Priority = Priority(220);
pub const PLACING_CONSTRUCTION_SITES_PRIORITY: Priority = Priority(100);
pub const CREEP_REGISTRATION_PRIORITY: Priority = Priority(220);
pub const ROOM_MAINTENANCE_PRIORITY: Priority = Priority(200);
pub const MOVE_CREEPS_PRIORITY: Priority = Priority(50);
pub const SPAWNING_CREEPS_PRIORITY: Priority = Priority(40);
pub const VISUALIZATIONS_PRIORITY: Priority = Priority(10);

pub const MINER_SPAWN_PRIORITY: Priority = Priority(200);
pub const HAULER_SPAWN_PRIORITY: Priority = Priority(150);
pub const UPGRADER_SPAWN_PRIORITY: Priority = Priority(100);
pub const BUILDER_SPAWN_PRIORITY: Priority = Priority(50);