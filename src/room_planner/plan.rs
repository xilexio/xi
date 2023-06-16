use crate::algorithms::room_matrix::RoomMatrix;
use crate::room_planner::planned_tile::PlannedTile;
use derive_more::Constructor;
use screeps::RoomXY;
use std::cmp::Ordering;
use std::fmt::Debug;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone, Constructor)]
pub struct Plan {
    pub tiles: RoomMatrix<PlannedTile>,
    pub controller: PlannedControllerData,
    pub sources: Vec<PlannedSourceData>,
    pub mineral: PlannedMineralData,
    pub score: PlanScore,
}

#[derive(Deserialize, Serialize, Debug, Clone, Copy, Default)]
pub struct PlannedControllerData {
    pub work_xy: RoomXY,
    pub link_xy: RoomXY,
}

#[derive(Deserialize, Serialize, Debug, Clone, Copy, Default)]
pub struct PlannedSourceData {
    pub source_xy: RoomXY,
    pub work_xy: RoomXY,
    pub link_xy: RoomXY,
}

#[derive(Deserialize, Serialize, Debug, Clone, Copy, Default)]
pub struct PlannedMineralData {
    pub work_xy: RoomXY,
}

#[derive(Deserialize, Serialize, Copy, Clone, PartialEq, Default, Debug)]
pub struct PlanScore {
    pub total_score: f32,
    pub energy_balance: f32,
    pub cpu_cost: f32,
    pub def_score: f32,
}

impl Eq for PlanScore {}

impl PartialOrd<Self> for PlanScore {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.total_score.partial_cmp(&other.total_score)
    }
}

impl Ord for PlanScore {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap_or(Ordering::Equal)
    }
}
