use std::cmp::Ordering;
use std::fmt::{Debug, Formatter};
use derive_more::Constructor;
use screeps::RoomXY;
use crate::algorithms::room_matrix::RoomMatrix;
use crate::room_planner::planned_tile::PlannedTile;

#[derive(Clone, Constructor)]
pub struct Plan {
    pub tiles: RoomMatrix<PlannedTile>,
    pub controller: PlannedControllerInfo,
    pub sources: Vec<PlannedSourceInfo>,
    pub mineral: PlannedMineralInfo,
    pub score: PlanScore,
}

#[derive(Clone, Copy, Default, Constructor)]
pub struct PlannedControllerInfo {
    pub link_xy: RoomXY,
    pub work_xy: RoomXY,
}

#[derive(Clone, Copy, Default, Constructor)]
pub struct PlannedSourceInfo {
    pub source_xy: RoomXY,
    pub link_xy: RoomXY,
    pub work_xy: RoomXY,
}

#[derive(Clone, Copy, Default, Constructor)]
pub struct PlannedMineralInfo {
    pub work_xy: RoomXY,
}

impl Debug for Plan {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let planned_titles_display = format!("{}", self.tiles);
        write!(f, "Plan {{ planned_titles:\n{}\n}}", planned_titles_display)
    }
}

#[derive(Copy, Clone, PartialEq, Default, Debug, Constructor)]
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