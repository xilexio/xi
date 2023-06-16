use derive_more::Constructor;
use crate::room_state::packed_terrain::PackedTerrain;
use js_sys::{Object, Reflect};
use log::info;
use screeps::{game, Mineral, ObjectId, ResourceType, RoomName, RoomXY, Source, StructureController, StructureType};
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::{JsCast, JsValue};
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use crate::room_planner::plan::Plan;
use crate::room_planner::RoomPlanner;

pub mod packed_terrain;
pub mod room_states;
pub mod scan_room;
pub mod scan_rooms;

// TODO Make it serializable and put in memory in serialized form.
#[derive(Deserialize, Serialize)]
pub struct RoomState {
    pub room_name: RoomName,
    pub owner: String,
    pub designation: RoomDesignation,
    pub rcl: u8,
    // TODO should not really be skipped
    #[serde(skip_serializing, skip_deserializing)]
    pub terrain: PackedTerrain,
    pub controller: Option<ControllerData>,
    pub sources: Vec<SourceData>,
    pub mineral: Option<MineralData>,
    // TODO ids of structures for owned rooms, where extensions and spawns and links are split by location, e.g., fastFillerExtensions
    // TODO for unowned rooms, ids are not as important (if at all)
    pub structures: StructuresMap,
    pub plan: Option<Plan>,
    #[serde(skip_serializing, skip_deserializing)]
    pub planner: Option<Box<RoomPlanner>>,
    /// Structures to be built at current RCL.
    pub current_rcl_structures: Option<StructuresMap>,
    pub current_rcl_structures_built: bool,
    // Information about fast filler and its extensions.
    // pub fast_filler: Option<FastFiller>,
    // Information about extensions outside of fast filler, ordered by the distance to the storage.
    // pub outer_extensions: Option<Vec<Extension>>,
    // Information about types and numbers of creeps to be regularly spawned.
    // pub spawn_schedule: Option<SpawnSchedule>,
}

#[derive(Deserialize, Serialize, Copy, Clone, Eq, PartialEq, Debug)]
pub enum RoomDesignation {
    Owned,
    NotOwned,
    Enemy,
    Invader,
    Portal,
    Highway
}

#[derive(Deserialize, Serialize, Copy, Clone, Debug, Constructor)]
pub struct ControllerData {
    pub id: ObjectId<StructureController>,
    pub xy: RoomXY,
    pub work_xy: Option<RoomXY>,
    pub link_xy: Option<RoomXY>,
}

#[derive(Deserialize, Serialize, Copy, Clone, Debug, Constructor)]
pub struct SourceData {
    pub id: ObjectId<Source>,
    pub xy: RoomXY,
    pub work_xy: Option<RoomXY>,
    pub link_xy: Option<RoomXY>,
}

#[derive(Deserialize, Serialize, Copy, Clone, Debug, Constructor)]
pub struct MineralData {
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
    pub fn new(room_name: RoomName) -> Self {
        RoomState {
            room_name,
            owner: String::new(),
            designation: RoomDesignation::NotOwned,
            rcl: 0,
            terrain: PackedTerrain::new(),
            controller: None,
            sources: Vec::new(),
            mineral: None,
            current_rcl_structures: None,
            current_rcl_structures_built: true,
            structures: FxHashMap::default(),
            plan: None,
            planner: None,
        }
    }
}

fn packed_terrain(room_state: &RoomState) -> PackedTerrain {
    game::map::get_room_terrain(room_state.room_name).into()
}