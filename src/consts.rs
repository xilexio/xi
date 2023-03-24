use screeps::ROOM_SIZE;

pub const ROOM_AREA: usize = (ROOM_SIZE as usize) * (ROOM_SIZE as usize);

pub const OBSTACLE_COST: u8 = 255;
pub const UNREACHABLE_COST: u8 = 254;