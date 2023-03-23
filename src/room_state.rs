use std::collections::HashMap;
use log::info;
use screeps::{Mineral, ObjectId, ResourceType, RoomName, RoomXY, Source, StructureController, StructureType};
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::{JsCast, JsValue};
use crate::room_state::packed_terrain::PackedTerrain;

pub mod scan;
mod room_states;
pub mod packed_terrain;

// TODO make it serializable and put in memory in serialized form
pub struct RoomState {
    pub name: RoomName,
    pub owner: String,
    pub designation: RoomDesignation,
    pub rcl: u8,
    pub terrain: PackedTerrain,
    pub controller: Option<ControllerInfo>,
    pub sources: Vec<SourceInfo>,
    pub mineral: Option<MineralInfo>,
    // TODO ids of buildings for owned rooms, where extensions and spawns and links are split by location, e.g., fastFillerExtensions
    // TODO for unowned rooms, ids are not as important (if at all)
    pub buildings: Buildings,
    pub plan: Option<Plan>,
}

pub enum RoomDesignation {
    OwnedRoom,
    PlayerRoom,
    NotOwnedRoom,
    NeutralRoom,
}

pub struct ControllerInfo {
    pub id: ObjectId<StructureController>,
    pub xy: RoomXY,
}

pub struct SourceInfo {
    pub id: ObjectId<Source>,
    pub xy: RoomXY,
}

pub struct MineralInfo {
    pub id: ObjectId<Mineral>,
    pub xy: RoomXY,
    pub mineral_type: ResourceType,
}

pub type Buildings = HashMap<StructureType, Vec<RoomXY>>;

pub struct Plan {
    pub rcl: u8,
    pub score: i16,
    pub buildings: Buildings,
}

#[wasm_bindgen]
pub fn set_room_blueprint(room_name: String, blueprint: JsValue) {
    info!("Room name: {}", room_name);

    // let blueprint_obj: &Object = blueprint.unchecked_ref();
    // let buildings = Reflect::get(&blueprint, &"buildings".into()).unwrap();
    // for structure_type in Reflect::own_keys(&buildings).unwrap().iter() {
    //     info!("{}:", structure_type.as_string().unwrap());
    //     let xy_array = Reflect::get(&buildings, &structure_type).unwrap();
    //     let length = Reflect::get(&xy_array, &"length".into()).unwrap().as_f64().unwrap();
    //     for i in 0..(length as u32) {
    //         let xy = Reflect::get_u32(&xy_array, i).unwrap();
    //         let x = Reflect::get(&xy, &"x".into()).unwrap().as_f64().unwrap();
    //         let y = Reflect::get(&xy, &"y".into()).unwrap().as_f64().unwrap();
    //         info!("({}, {})", x, y);
    //     }
    // };
}

impl RoomState {
    pub fn new(name: RoomName) -> Self {
        RoomState {
            name,
            owner: String::new(),
            designation: RoomDesignation::NotOwnedRoom,
            rcl: 0,
            terrain: PackedTerrain::new(),
            controller: None,
            sources: Vec::new(),
            mineral: None,
            buildings: HashMap::new(),
            plan: None,
        }
    }
}
