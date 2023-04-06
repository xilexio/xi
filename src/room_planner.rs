use std::cmp::{max, min};
use crate::algorithms::distance_matrix::distance_matrix;
use crate::room_planner::plan::Plan;
use crate::room_planner::RoomPlannerError::{ControllerNotFound, SourceNotFound, UnreachablePOI};
use crate::room_state::{RoomState};
use std::error::Error;
use std::iter::once;
use log::debug;
use screeps::StructureType::{Rampart, Road};
use thiserror::Error;
use crate::algorithms::grid_min_cut::grid_min_cut;
use crate::algorithms::matrix_common::MatrixCommon;
use crate::algorithms::shortest_path_by_matrix::shortest_path_by_matrix;
use crate::algorithms::room_matrix::RoomMatrix;
use crate::consts::{OBSTACLE_COST, UNREACHABLE_COST};
use crate::room_planner::packed_tile_structures::PackedTileStructures;

pub mod plan;
pub mod packed_tile_structures;

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
    for source_dm in source_dms.iter() {
        dist_sum.update(move |xy, value| {
            let source_value = source_dm.get(xy);
            if value >= UNREACHABLE_COST || source_value >= UNREACHABLE_COST {
                max(value, source_value)
            } else {
                min(value.saturating_add(source_value), UNREACHABLE_COST - 1)
            }
        });
    }

    let (min_dist_xy, min_dist_sum) = dist_sum.min();
    if min_dist_sum == UNREACHABLE_COST {
        Err(UnreachablePOI)?
    }

    let mut buildings_matrix = RoomMatrix::new(PackedTileStructures::default());
    let mut midpoint_roads_min_cut_matrix = RoomMatrix::new(1);
    for &xy in walls.iter() {
        midpoint_roads_min_cut_matrix.set(xy, OBSTACLE_COST);
    }

    debug!("Triple point in {}.", min_dist_xy);

    for dm in once(&controller_dm).chain(source_dms.iter()) {
        for xy in shortest_path_by_matrix(dm, min_dist_xy, 1).into_iter() {
            buildings_matrix.set(xy, Road.into());
            midpoint_roads_min_cut_matrix.set(xy, 0);
        }
    }

    let midpoint_roads_min_cut = grid_min_cut(midpoint_roads_min_cut_matrix);

    for &xy in midpoint_roads_min_cut.iter() {
        buildings_matrix.set(xy, Rampart.into());
    }

    // TODO find places that have minimal max distance to all ramparts

    Ok(Plan {
        buildings: buildings_matrix.to_structures_map(),
    })
}
