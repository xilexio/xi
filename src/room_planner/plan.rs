use std::cmp::Ordering;
use std::fmt::{Debug, Formatter};
use derive_more::Constructor;
use crate::algorithms::room_matrix::RoomMatrix;
use crate::room_planner::planned_tile::PlannedTile;

#[derive(Clone, Default, Constructor)]
pub struct Plan {
    pub planned_tiles: RoomMatrix<PlannedTile>,
    pub score: PlanScore,
}

impl Debug for Plan {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let planned_titles_display = format!("{}", self.planned_tiles);
        write!(f, "Plan {{ planned_titles:\n{}\n}}", planned_titles_display)
    }
}

#[derive(Copy, Clone, PartialEq, Default, Debug)]
pub struct PlanScore {
    pub total_score: f32,
    pub eco_score: f32,
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