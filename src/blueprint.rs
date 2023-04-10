use crate::room_state::{StructuresMap, MineralInfo, SourceInfo};
use screeps::RoomXY;
use rustc_hash::FxHashMap;

pub struct Blueprint {
    pub name: String,
    pub rcl: u8,
    pub walls: Vec<RoomXY>,
    pub swamps: Vec<RoomXY>,
    pub controller: Option<RoomXY>,
    pub sources: Vec<SourceInfo>,
    pub mineral: Option<MineralInfo>,
    pub structures: StructuresMap,
}

impl Blueprint {
    pub fn new() -> Self {
        Blueprint {
            name: String::new(),
            rcl: 0,
            walls: Vec::new(),
            swamps: Vec::new(),
            controller: None,
            sources: Vec::new(),
            mineral: None,
            structures: FxHashMap::default(),
        }
    }
}
