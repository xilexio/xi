use derive_more::Constructor;
use crate::room_state::packed_terrain::PackedTerrain;
use js_sys::{Object, Reflect};
use log::info;
use screeps::{game, ConstructionSite, Mineral, ObjectId, ResourceType, RoomName, RoomXY, Source, StructureContainer, StructureController, StructureExtension, StructureLink, StructureSpawn, StructureType};
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::{JsCast, JsValue};
use rustc_hash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};
use crate::creeps::CreepRef;
use crate::hauling::hauling_stats::HaulingStats;
use crate::kernel::broadcast::Broadcast;
use crate::room_planner::plan::Plan;
use crate::room_planner::RoomPlanner;
use crate::u;

pub mod packed_terrain;
pub mod room_states;
pub mod scan_room;
pub mod scan_rooms;
pub mod utils;

#[derive(Debug, Deserialize, Serialize)]
pub struct RoomState {
    pub room_name: RoomName,
    pub owner: String,
    pub designation: RoomDesignation,
    pub rcl: u8,
    #[serde(skip)]
    pub terrain: PackedTerrain,
    pub controller: Option<ControllerData>,
    pub sources: Vec<SourceData>,
    pub mineral: Option<MineralData>,
    // TODO ids of structures for owned rooms, where extensions and spawns and links are split by location, e.g., fastFillerExtensions
    // TODO for unowned rooms, ids are not as important (if at all)
    pub structures: StructuresMap,
    pub plan: Option<Plan>,
    #[serde(skip)]
    pub planner: Option<Box<RoomPlanner>>,
    /// Structures to be built at current RCL.
    pub current_rcl_structures: Option<StructuresMap>,
    /// Indicator whether all structures required in the current RCL are built. Used to trigger construction.
    pub current_rcl_structures_built: bool,
    #[serde(skip)]
    pub construction_site_queue: Vec<ConstructionSiteData>,
    // Information about fast filler and its extensions.
    // pub fast_filler: Option<FastFiller>,
    // Information about extensions outside of fast filler, ordered by the distance to the storage.
    // pub outer_extensions: Option<Vec<Extension>>,
    #[serde(skip)]
    pub spawns: Vec<StructureData<StructureSpawn>>,
    #[serde(skip)]
    pub extensions: Vec<StructureData<StructureExtension>>,
    /// Broadcast signalled each time the set of structures in the room changes.
    #[serde(skip)]
    pub structures_broadcast: Broadcast<()>,
    #[serde(skip)]
    pub resources: RoomResources,
    #[serde(skip)]
    pub essential_creeps: EssentialCreeps,
    pub hauling_stats: HaulingStats,
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

#[derive(Clone, Debug)]
pub struct ConstructionSiteData {
    pub id: ObjectId<ConstructionSite>,
    pub structure_type: StructureType,
    pub xy: RoomXY,
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
    pub container_id: Option<ObjectId<StructureContainer>>,
    pub link_xy: Option<RoomXY>,
    pub link_id: Option<ObjectId<StructureLink>>,
}

#[derive(Deserialize, Serialize, Copy, Clone, Debug, Constructor)]
pub struct MineralData {
    pub id: ObjectId<Mineral>,
    pub xy: RoomXY,
    pub mineral_type: ResourceType,
}

#[derive(Debug, Clone, Constructor)]
pub struct StructureData<T> {
    pub id: ObjectId<T>,
    pub xy: RoomXY,
}

pub type StructuresMap = FxHashMap<StructureType, FxHashSet<RoomXY>>;

#[derive(Default, Clone, Debug)]
pub struct RoomResources {
    pub spawn_energy: u32,
    pub spawn_energy_capacity: u32,
    pub storage_energy: u32,
}

/// List of creeps essential for the continued and uninterrupted function of the room.
/// The creeps that are important depend on the room's RCL.
/// The bot tries to keep at least one of each required essential creep type with plenty of
/// ticks to live to restart the room if necessary.
#[derive(Default, Clone, Debug)]
pub struct EssentialCreeps {
    miner: Option<CreepRef>,
    hauler: Option<CreepRef>,
}

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
            construction_site_queue: Vec::new(),
            spawns: Vec::new(),
            extensions: Vec::new(),
            structures_broadcast: Broadcast::default(),
            resources: RoomResources::default(),
            essential_creeps: EssentialCreeps::default(),
            hauling_stats: HaulingStats::default(),
        }
    }
}

fn packed_terrain(room_state: &RoomState) -> PackedTerrain {
    u!(game::map::get_room_terrain(room_state.room_name)).into()
}