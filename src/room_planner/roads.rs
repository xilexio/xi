use crate::algorithms::distance_matrix::targeted_distance_matrix;
use crate::algorithms::matrix_common::MatrixCommon;
use crate::algorithms::room_matrix::RoomMatrix;
use crate::algorithms::shortest_path_by_matrix::shortest_path_by_matrix_with_preference;
use crate::algorithms::weighted_distance_matrix::{obstacle_cost, weighted_distance_matrix};
use crate::room_planner::packed_tile_structures::PackedTileStructures;
use crate::room_planner::RoomPlannerError;
use crate::room_state::packed_terrain::PackedTerrain;
use screeps::RoomXY;
use screeps::StructureType::Road;
use screeps::Terrain::{Swamp, Wall};

const PLAIN_ROAD_COST: u16 = 100;
const SWAMP_ROAD_COST: u16 = 101;
const EXISTING_ROAD_COST: u16 = 75;

pub fn connect_with_roads(
    terrain: &PackedTerrain,
    structures_matrix: &mut RoomMatrix<PackedTileStructures>,
    start: impl Iterator<Item = RoomXY>,
    targets: impl Iterator<Item = RoomXY>,
    storage_xy: RoomXY,
) -> Result<(), RoomPlannerError> {
    let grid_bit = (storage_xy.x.u8() + storage_xy.y.u8()) % 2;
    let mut cost_matrix = RoomMatrix::new(PLAIN_ROAD_COST);
    let mut checkerboard = RoomMatrix::new(0u8);
    for (xy, t) in terrain.iter() {
        checkerboard.set(xy, [0, 1][((grid_bit + xy.x.u8() + xy.y.u8()) % 2) as usize]);
        if t == Wall {
            cost_matrix.set(xy, obstacle_cost());
        } else if t == Swamp {
            cost_matrix.set(xy, SWAMP_ROAD_COST);
        }
    }
    for (xy, structure) in structures_matrix.iter() {
        if !structure.road().is_empty() {
            cost_matrix.set(xy, EXISTING_ROAD_COST);
        } else if !structure.is_passable(true) {
            cost_matrix.set(xy, obstacle_cost());
        }
    }

    let start_vec = start.collect::<Vec<_>>();

    for target in targets {
        // TODO it should be less expensive to recompute it using the existing cost matrix as base, at least on the side further away from added roads than from previously existing ones
        let distances = weighted_distance_matrix(&cost_matrix, start_vec.iter().copied());
        // TODO for now it is a small optimization that tries to merge roads, but a proper merging should occur using Steiner Minimal Tree algorithm
        // TODO .ok_or(RoadConnectionFailure)? if we cannot get within final_dist
        // TODO final_dist here does not work as intended
        let mut path = shortest_path_by_matrix_with_preference(&distances, &checkerboard, target, 0);
        path.pop();
        for &xy in path.iter().skip(1) {
            structures_matrix.set(xy, structures_matrix.get(xy).with(Road));
            cost_matrix.set(xy, EXISTING_ROAD_COST);
        }
    }

    Ok(())
}
