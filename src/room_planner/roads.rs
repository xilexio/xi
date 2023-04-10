use crate::algorithms::distance_matrix::targeted_distance_matrix;
use crate::algorithms::matrix_common::MatrixCommon;
use crate::algorithms::room_matrix::RoomMatrix;
use crate::algorithms::shortest_path_by_matrix::shortest_path_by_matrix_with_preference;
use crate::room_planner::packed_tile_structures::PackedTileStructures;
use crate::room_planner::RoomPlannerError;
use crate::room_planner::RoomPlannerError::RoadConnectionFailure;
use screeps::RoomXY;
use screeps::StructureType::Road;

pub fn connect_with_roads(
    walls: impl Iterator<Item = RoomXY>,
    structures_matrix: &mut RoomMatrix<PackedTileStructures>,
    start: impl Iterator<Item = RoomXY>,
    targets: impl Iterator<Item = RoomXY>,
) -> Result<(), RoomPlannerError> {
    let obstacles = walls.chain(
        structures_matrix
            .iter()
            .filter_map(|(xy, structures)| (!structures.is_passable(true)).then_some(xy)),
    );
    let targets_vec = targets.collect::<Vec<_>>();
    let distances =
        targeted_distance_matrix(obstacles, start, targets_vec.iter().copied()).ok_or(RoadConnectionFailure)?;

    let mut no_roads = structures_matrix.map(|xy, structure| structure.road().is_empty());

    for target in targets_vec.into_iter() {
        // TODO for now it is a small optimization that tries to merge roads, but a proper merging should occur using Steiner Minimal Tree algorithm
        let path = shortest_path_by_matrix_with_preference(&distances, &no_roads, target, 1);
        for &xy in path.iter().skip(1) {
            structures_matrix.set(xy, structures_matrix.get(xy).with(Road));
            no_roads.set(xy, false);
        }
    }

    Ok(())
}
