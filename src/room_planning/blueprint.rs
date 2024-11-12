use screeps::RoomXY;
use rustc_hash::FxHashMap;
use crate::room_states::room_state::{MineralData, SourceData, StructuresMap};

pub struct Blueprint {
    pub name: String,
    pub rcl: u8,
    pub walls: Vec<RoomXY>,
    pub swamps: Vec<RoomXY>,
    pub controller: Option<RoomXY>,
    pub sources: Vec<SourceData>,
    pub mineral: Option<MineralData>,
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
