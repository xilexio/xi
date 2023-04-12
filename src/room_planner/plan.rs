use std::fmt::{Debug, Formatter};
use crate::algorithms::room_matrix::RoomMatrix;
use crate::room_planner::planned_tile::PlannedTile;

#[derive(Clone)]
pub struct Plan {
    pub planned_tiles: RoomMatrix<PlannedTile>,
}

impl Debug for Plan {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let planned_titles_display = format!("{}", self.planned_tiles);
        write!(f, "Plan {{ planned_titles:\n{}\n}}", planned_titles_display)
    }
}