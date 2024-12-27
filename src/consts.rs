use screeps::ROOM_SIZE;

pub const ROOM_AREA: usize = (ROOM_SIZE as usize) * (ROOM_SIZE as usize);

pub const OBSTACLE_COST: u8 = 255;
pub const UNREACHABLE_COST: u8 = 254;

/// Cost of repairing something with a single `Work` part.
pub const REPAIR_COST_PER_PART: u32 = 1;

pub const FAR_FUTURE: u32 = 1_000_000_000;