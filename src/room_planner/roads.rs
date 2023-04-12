use log::debug;
use crate::algorithms::matrix_common::MatrixCommon;
use crate::algorithms::room_matrix::RoomMatrix;
use crate::algorithms::shortest_path_by_matrix::shortest_path_by_matrix_with_preference;
use crate::algorithms::weighted_distance_matrix::{obstacle_cost, weighted_distance_matrix};
use crate::room_planner::planned_tile::PlannedTile;
use crate::room_planner::RoomPlannerError;
use crate::room_state::packed_terrain::PackedTerrain;
use derive_more::Constructor;
use screeps::RoomXY;
use screeps::StructureType::Road;
use screeps::Terrain::{Swamp, Wall};

const PLAIN_ROAD_COST: u16 = 100;
const SWAMP_ROAD_COST: u16 = 101;
const EXISTING_ROAD_COST: u16 = 75;

#[derive(Copy, Clone, Debug, Constructor)]
pub struct RoadTarget {
    xy: RoomXY,
    stop_dist: u8,
    interior: bool,
}

pub fn connect_with_roads(
    terrain: &PackedTerrain,
    structures_matrix: &mut RoomMatrix<PlannedTile>,
    start: impl Iterator<Item = RoomXY>,
    start_dist: u8,
    targets: impl Iterator<Item = RoadTarget>,
    storage_xy: RoomXY,
) -> Result<(), RoomPlannerError> {
    let grid_bit = (storage_xy.x.u8() + storage_xy.y.u8()) % 2;
    let mut checkerboard = RoomMatrix::new(0u8);
    let mut cost_matrix = RoomMatrix::new(PLAIN_ROAD_COST);
    for (xy, t) in terrain.iter() {
        checkerboard.set(xy, [0, 1][((grid_bit + xy.x.u8() + xy.y.u8()) % 2) as usize]);
        if t == Wall {
            cost_matrix.set(xy, obstacle_cost());
        } else if t == Swamp {
            cost_matrix.set(xy, SWAMP_ROAD_COST);
        }
    }
    for (xy, tile) in structures_matrix.iter() {
        if !tile.is_passable(true) {
            cost_matrix.set(xy, obstacle_cost());
        } else if tile.structures().road() {
            cost_matrix.set(xy, EXISTING_ROAD_COST);
        }
    }

    let start_vec = start.collect::<Vec<_>>();

    for target in targets {
        // TODO it should be less expensive to recompute it using the existing cost matrix as base, at least on the side further away from added roads than from previously existing ones
        let distances = weighted_distance_matrix(&cost_matrix, start_vec.iter().copied());
        // TODO for now it is a small optimization that tries to merge roads, but a proper merging should occur using Steiner Minimal Tree algorithm
        // TODO .ok_or(RoadConnectionFailure)? if we cannot get within final_dist
        // TODO final_dist here does not work as intended
        debug!("connect_with_roads from {:?} to {:?}", start_vec, target);
        let path = shortest_path_by_matrix_with_preference(&distances, &checkerboard, target.xy);
        debug!("path: {:?}", path);
        for &xy in &path[(target.stop_dist as usize)..(path.len() - (start_dist as usize))] {
            let mut tile = structures_matrix.get(xy).with(Road);
            if target.interior {
                tile = tile.with_interior(true);
            }
            debug!("{} before {:?} after {:?}", xy, structures_matrix.get(xy), tile);
            structures_matrix.set(xy, tile);
            cost_matrix.set(xy, EXISTING_ROAD_COST);
        }
    }

    Ok(())
}
