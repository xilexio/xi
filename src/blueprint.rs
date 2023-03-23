use std::collections::HashMap;
use screeps::{RoomXY, StructureType};
use crate::room_state::{Buildings, MineralInfo, SourceInfo};

pub struct Blueprint {
    pub name: String,
    pub rcl: u8,
    pub walls: Vec<RoomXY>,
    pub swamps: Vec<RoomXY>,
    pub controller: Option<RoomXY>,
    pub sources: Vec<SourceInfo>,
    pub mineral: Option<MineralInfo>,
    pub buildings: Buildings,
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
            buildings: HashMap::new(),
        }
    }
}