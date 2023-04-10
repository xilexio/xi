use crate::room_state::packed_terrain::PackedTerrain;
use js_sys::{Object, Reflect};
use log::info;
use screeps::{
    Mineral, ObjectId, ResourceType, RoomName, RoomXY, Source, StructureController, StructureType,
};
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::{JsCast, JsValue};
use rustc_hash::FxHashMap;
use crate::room_planner::plan::Plan;

pub mod packed_terrain;
pub mod room_states;
pub mod scan;

// TODO make it serializable and put in memory in serialized form
#[derive(Clone, Debug)]
pub struct RoomState {
    pub name: RoomName,
    pub owner: String,
    pub designation: RoomDesignation,
    pub rcl: u8,
    pub terrain: PackedTerrain,
    pub controller: Option<ControllerInfo>,
    pub sources: Vec<SourceInfo>,
    pub mineral: Option<MineralInfo>,
    // TODO ids of structures for owned rooms, where extensions and spawns and links are split by location, e.g., fastFillerExtensions
    // TODO for unowned rooms, ids are not as important (if at all)
    pub structures: StructuresMap,
    pub plan: Option<Plan>,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum RoomDesignation {
    OwnedRoom,
    PlayerRoom,
    NotOwnedRoom,
    NeutralRoom,
}

#[derive(Copy, Clone, Debug)]
pub struct ControllerInfo {
    pub id: ObjectId<StructureController>,
    pub xy: RoomXY,
}

#[derive(Copy, Clone, Debug)]
pub struct SourceInfo {
    pub id: ObjectId<Source>,
    pub xy: RoomXY,
}

#[derive(Copy, Clone, Debug)]
pub struct MineralInfo {
    pub id: ObjectId<Mineral>,
    pub xy: RoomXY,
    pub mineral_type: ResourceType,
}

pub type StructuresMap = FxHashMap<StructureType, Vec<RoomXY>>;

#[wasm_bindgen]
pub fn set_room_blueprint(room_name: String, blueprint: JsValue) {
    info!("Room name: {}", room_name);

    let blueprint_obj: &Object = blueprint.unchecked_ref();
    let structures = Reflect::get(&blueprint, &"buildings".into()).unwrap();
    for structure_type in Reflect::own_keys(&structures).unwrap().iter() {
        info!("{}:", structure_type.as_string().unwrap());
        let xy_array = Reflect::get(&structures, &structure_type).unwrap();
        let length = Reflect::get(&xy_array, &"length".into())
            .unwrap()
            .as_f64()
            .unwrap();
        for i in 0..(length as u32) {
            let xy = Reflect::get_u32(&xy_array, i).unwrap();
            let x = Reflect::get(&xy, &"x".into()).unwrap().as_f64().unwrap();
            let y = Reflect::get(&xy, &"y".into()).unwrap().as_f64().unwrap();
            info!("({}, {})", x, y);
        }
    }
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
            structures: FxHashMap::default(),
            plan: None,
        }
    }
}
