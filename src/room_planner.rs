use std::cmp::{max, min};
use crate::algorithms::distance_matrix::distance_matrix;
use crate::room_planner::plan::Plan;
use crate::room_planner::RoomPlannerError::{ControllerNotFound, SourceNotFound, UnreachablePOI};
use crate::room_state::{Buildings, RoomState};
use std::error::Error;
use std::iter::once;
use thiserror::Error;
use crate::algorithms::matrix_common::MatrixCommon;
use crate::consts::UNREACHABLE_COST;

pub mod plan;

#[derive(Error, Debug)]
pub enum RoomPlannerError {
    #[error("controller not found")]
    ControllerNotFound,
    #[error("at least one source not found")]
    SourceNotFound,
    #[error("one of sources, the mineral or the controller is unreachable")]
    UnreachablePOI,
}

pub fn plan_room(state: &RoomState) -> Result<Plan, Box<dyn Error>> {
    let controller = state.controller.ok_or(ControllerNotFound)?;
    if state.sources.is_empty() {
        Err(SourceNotFound)?;
    }
    let sources = state.sources.clone();

    let walls = state.terrain.walls().collect::<Vec<_>>();
    let controller_dm = distance_matrix(once(controller.xy), walls.iter().copied());
    let source_dms = sources
        .iter()
        .map(|source| distance_matrix(once(source.xy), walls.iter().copied()))
        .collect::<Vec<_>>();

    let mut dist_sum = controller_dm.clone();
    for source_dm in source_dms {
        dist_sum.update(move |xy, value| {
            let source_value = source_dm.get(xy);
            if value >= UNREACHABLE_COST || source_value >= UNREACHABLE_COST {
                max(value, source_value)
            } else {
                min(value.saturating_add(source_value), UNREACHABLE_COST - 1)
            }
        });
    }

    let min_dist_sum = dist_sum.iter().fold(UNREACHABLE_COST, |current_min, (_, value)| {
       min(current_min, value)
    });
    if min_dist_sum == UNREACHABLE_COST {
        Err(UnreachablePOI)?
    }

    // TODO join the three points by a road
    // TODO min-cut for this road
    // TODO visualize this
    // TODO find places that have minimal max distance to all ramparts

    Ok(Plan {
        buildings: Buildings::default(),
    })
}
