use screeps::Position;
use serde::{Deserialize, Serialize};
use crate::creeps::creep::CreepBody;

/// A structure gathering energy, transportation throughput and other statistics to decide on
/// the distribution of resources in the room, e.g., on the number of haulers, upgraders, etc.
#[derive(Debug)]
pub struct RoomEcoStats {
    /// Places from which energy is generated and hauled if there is no storage.
    pub energy_sources: Vec<RecurrentResourceChange>,
    /// Places to which energy needs to be hauled to build a structure from the construction site
    /// queue.
    pub construction_sites: Vec<OneTimeResourceChange>,
    pub storage_pos: Option<Position>,
    pub hauler_body: CreepBody,
    pub miner_body: CreepBody,
    pub number_of_haulers: u32,
    pub number_of_miners: u32,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RecurrentResourceChange {
    pub change_per_tick: f32,
    pub pos: Position,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct OneTimeResourceChange {
    pub change: f32,
    pub pos: Position,
}