use std::iter::{Flatten, Map};
use std::option::IntoIter;
use serde::{Deserialize, Serialize};
use derive_more::Constructor;
use screeps::{
    game,
    Mineral,
    ObjectId,
    Position,
    RawObjectId,
    ResourceType,
    RoomName,
    RoomXY,
    Source,
    Structure,
    StructureContainer,
    StructureController,
    StructureLink,
    StructureType,
    Terrain,
};
use rustc_hash::{FxHashMap, FxHashSet};
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::{JsCast, JsValue};
use log::info;
use js_sys::{Object, Reflect};
use crate::algorithms::matrix_common::MatrixCommon;
use crate::algorithms::room_matrix::RoomMatrix;
use crate::construction::place_construction_sites::ConstructionSiteData;
use crate::construction::triage_repair_sites::{StructureToRepair, TriagedRepairSites};
use crate::creeps::creeps::CreepRef;
use crate::economy::room_eco_config::RoomEcoConfig;
use crate::economy::room_eco_stats::RoomEcoStats;
use crate::geometry::room_xy::RoomXYUtils;
use crate::kernel::broadcast::Broadcast;
use crate::room_planning::packed_tile_structures::PackedTileStructures;
use crate::room_planning::plan::Plan;
use crate::room_planning::room_planner::RoomPlanner;
use crate::room_states::packed_terrain::PackedTerrain;
use crate::travel::surface::Surface;
use crate::u;

// TODO Instead of Option everywhere, create OwnedRoomState with all extra attributes or even better,
//      combine it with designation into one enum.
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
    #[serde(skip)]
    pub structures: FxHashMap<StructureType, FxHashMap<RoomXY, ObjectId<Structure>>>,
    #[serde(skip)]
    pub structures_matrix: RoomMatrix<PackedTileStructures>,
    pub plan: Option<Plan>,
    #[serde(skip)]
    pub planner: Option<Box<RoomPlanner>>,
    /// Structures to be built at current RCL.
    pub current_rcl_structures: StructuresMap,
    #[serde(skip)]
    pub extra_construction_sites: Vec<ConstructionSiteData>,
    #[serde(skip)]
    pub construction_site_queue: Vec<ConstructionSiteData>,
    #[serde(skip)]
    pub structures_to_repair: FxHashMap<StructureType, Vec<StructureToRepair>>,
    #[serde(skip)]
    pub triaged_repair_sites: TriagedRepairSites,
    // Information about fast filler and its extensions.
    // pub fast_filler: Option<FastFiller>,
    // Information about extensions outside of fast filler, ordered by the distance to the storage.
    // pub outer_extensions: Option<Vec<Extension>>,
    /// Broadcast signalled each time the set of structures in the room changes.
    #[serde(skip)]
    pub structures_broadcast: Broadcast<()>,
    #[serde(skip)]
    pub resources: RoomResources,
    #[serde(skip)]
    pub essential_creeps: Option<EssentialCreeps>,
    #[serde(skip)]
    pub eco_stats: Option<RoomEcoStats>,
    #[serde(skip)]
    pub eco_config: Option<RoomEcoConfig>,
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
    pub downgrade_tick: u32,
}

#[derive(Deserialize, Serialize, Clone, Debug, Constructor)]
pub struct SourceData {
    pub id: ObjectId<Source>,
    pub xy: RoomXY,
    /// The main work position that is next to a link and over a container.
    pub work_xy: Option<RoomXY>,
    /// The work positions available when drop mining.
    pub drop_mining_xys: Vec<RoomXY>,
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
            current_rcl_structures: FxHashMap::default(),
            structures: FxHashMap::default(),
            structures_matrix: RoomMatrix::default(),
            plan: None,
            planner: None,
            extra_construction_sites: Vec::new(),
            construction_site_queue: Vec::new(),
            structures_to_repair: FxHashMap::default(),
            triaged_repair_sites: TriagedRepairSites::default(),
            structures_broadcast: Broadcast::default(),
            resources: RoomResources::default(),
            essential_creeps: None,
            eco_stats: None,
            eco_config: None,
        }
    }

    /// Returns the `RoomXY` of the first structure of the given type.
    /// If there is more than one, an arbitrary one is chosen.
    pub fn structure_xy(&self, structure_type: StructureType) -> Option<RoomXY> {
        self.structures
            .get(&structure_type)
            .and_then(|structures_data| {
                structures_data.keys().next().cloned()
            })
    }

    /// Returns the `Position` of the first structure of the given type.
    /// If there is more than one, an arbitrary one is chosen.
    pub fn structure_pos(&self, structure_type: StructureType) -> Option<Position> {
        self.structure_xy(structure_type)
            .map(|xy| xy.to_pos(self.room_name))
    }

    // TODO The return type is ugly, change it to impl Iterator<Item = (RoomXY, RawObjectId)> + use<'_> later.
    pub fn structures_with_type<T>(&self, structure_type: StructureType) -> Map<Flatten<IntoIter<&FxHashMap<RoomXY, ObjectId<Structure>>>>, fn((&RoomXY, &ObjectId<Structure>)) -> (RoomXY, ObjectId<T>)> {
        self.structures
            .get(&structure_type)
            .into_iter()
            .flatten()
            .map(|(&xy, &id)| (xy, RawObjectId::from(id).into()))
    }
    
    pub fn planned_structure_pos(&self, structure_type: StructureType) -> Option<Position> {
        let plan = self.plan.as_ref()?;
        plan.tiles
            .find_structure_xys(structure_type)
            .first()
            .map(|xy| xy.to_pos(self.room_name))
    }

    pub fn tile_surface(&self, xy: RoomXY) -> Surface {
        let tile_structures = self.structures_matrix.get(xy);
        if tile_structures.road() {
            Surface::Road
        } else if !tile_structures.is_passable(self.designation == RoomDesignation::Owned) {
            Surface::Obstacle
        } else {
            // TODO This is ugly and inefficient. Include construction sites in structures_map,
            //      instead.
            if self.construction_site_queue.iter().any(|cs| cs.pos.room_name() == self.room_name && cs.pos.xy() == xy && cs.structure_type != StructureType::Container && cs.structure_type != StructureType::Road && cs.structure_type != StructureType::Rampart) {
                Surface::Obstacle
            } else {
                match self.terrain.get(xy) {
                    Terrain::Plain => {
                        Surface::Plain
                    }
                    Terrain::Wall => {
                        Surface::Obstacle
                    }
                    Terrain::Swamp => {
                        Surface::Swamp
                    }
                }
            }
        }
    }

    pub fn update_structures_matrix(&mut self) {
        self.structures_matrix = u!((&self.structures).try_into());
    }
}

fn packed_terrain(room_state: &RoomState) -> PackedTerrain {
    u!(game::map::get_room_terrain(room_state.room_name)).into()
}

#[cfg(test)]
pub fn empty_unowned_room_state() -> RoomState {
    RoomState::new(test_empty_unowned_room_name())
}

#[cfg(test)]
pub fn test_empty_unowned_room_name() -> RoomName {
    RoomName::new("W1N1").unwrap()
}