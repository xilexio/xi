use crate::algorithms::matrix_common::MatrixCommon;
use crate::algorithms::room_matrix::RoomMatrix;
use crate::algorithms::room_matrix_slice::RoomMatrixSlice;
use crate::consts::{OBSTACLE_COST, UNREACHABLE_COST};
use crate::geometry::rect::Rect;
use crate::geometry::room_xy::RoomXYUtils;
use screeps::RoomXY;
use std::cmp::min;

pub fn distance_matrix<S, O>(start: S, obstacles: O) -> RoomMatrix<u8>
where
    S: Iterator<Item = RoomXY>,
    O: Iterator<Item = RoomXY>,
{
    let mut result = RoomMatrix::new(UNREACHABLE_COST);

    for xy in obstacles {
        result.set(xy, OBSTACLE_COST);
    }

    let mut layer = Vec::new();

    for xy in start {
        result.set(xy, 0);
        layer.push(xy);
    }

    let mut distance = 1u8;

    while !layer.is_empty() {
        let mut next_layer = Vec::new();
        for xy in layer {
            for near in xy.around() {
                if result.get(near) == UNREACHABLE_COST {
                    result.set(near, distance);
                    next_layer.push(near);
                }
            }
        }
        layer = next_layer;
        distance = min(UNREACHABLE_COST - 1, distance + 1);
    }

    result
}

pub fn restricted_distance_matrix<S, O>(
    start: S,
    obstacles: O,
    slice: Rect,
    max_distance: u8,
) -> RoomMatrixSlice<u8>
where
    S: Iterator<Item = RoomXY>,
    O: Iterator<Item = RoomXY>,
{
    let mut result = RoomMatrixSlice::new(slice, UNREACHABLE_COST);

    for xy in obstacles {
        result.set(xy, OBSTACLE_COST);
    }

    let mut layer = Vec::new();

    for xy in start {
        result.set(xy, 0);
        layer.push(xy);
    }

    let mut distance = 1u8;

    while !layer.is_empty() && distance <= max_distance {
        let mut next_layer = Vec::new();
        for xy in layer {
            for near in xy.restricted_around(slice) {
                if result.get(near) == UNREACHABLE_COST {
                    result.set(near, distance);
                    next_layer.push(near);
                }
            }
        }
        layer = next_layer;
        distance = min(UNREACHABLE_COST - 1, distance + 1);
    }

    result
}

#[cfg(test)]
mod tests {
    use crate::algorithms::distance_matrix::restricted_distance_matrix;
    use crate::algorithms::matrix_common::MatrixCommon;
    use crate::consts::{OBSTACLE_COST, UNREACHABLE_COST};
    use crate::geometry::rect::Rect;
    use screeps::RoomXY;
    use std::error::Error;

    #[test]
    fn test_restricted_grid_bfs_distances() -> Result<(), Box<dyn Error>> {
        let slice = Rect::new(RoomXY::try_from((10, 10))?, RoomXY::try_from((12, 12))?)?;
        let dists = restricted_distance_matrix(
            [RoomXY::try_from((10, 10))?].into_iter(),
            [
                RoomXY::try_from((11, 11))?,
                RoomXY::try_from((12, 11))?,
                RoomXY::try_from((11, 12))?,
            ]
            .into_iter(),
            slice,
            10,
        );
        assert_eq!(dists.get(RoomXY::try_from((10, 10))?), 0);
        assert_eq!(dists.get(RoomXY::try_from((11, 10))?), 1);
        assert_eq!(dists.get(RoomXY::try_from((12, 10))?), 2);
        assert_eq!(dists.get(RoomXY::try_from((10, 11))?), 1);
        assert_eq!(dists.get(RoomXY::try_from((11, 11))?), OBSTACLE_COST);
        assert_eq!(dists.get(RoomXY::try_from((12, 11))?), OBSTACLE_COST);
        assert_eq!(dists.get(RoomXY::try_from((10, 12))?), 2);
        assert_eq!(dists.get(RoomXY::try_from((11, 12))?), OBSTACLE_COST);
        assert_eq!(dists.get(RoomXY::try_from((12, 12))?), UNREACHABLE_COST);
        Ok(())
    }

    #[test]
    fn test_restricted_grid_bfs_distances_with_max_distance() -> Result<(), Box<dyn Error>> {
        let slice = Rect::new(RoomXY::try_from((10, 10))?, RoomXY::try_from((12, 12))?)?;
        let dists = restricted_distance_matrix(
            [RoomXY::try_from((10, 10))?].into_iter(),
            [
                RoomXY::try_from((11, 11))?,
                RoomXY::try_from((12, 11))?,
                RoomXY::try_from((11, 12))?,
            ]
            .into_iter(),
            slice,
            1,
        );
        assert_eq!(dists.get(RoomXY::try_from((10, 10))?), 0);
        assert_eq!(dists.get(RoomXY::try_from((11, 10))?), 1);
        assert_eq!(dists.get(RoomXY::try_from((12, 10))?), UNREACHABLE_COST);
        assert_eq!(dists.get(RoomXY::try_from((10, 11))?), 1);
        assert_eq!(dists.get(RoomXY::try_from((11, 11))?), OBSTACLE_COST);
        assert_eq!(dists.get(RoomXY::try_from((12, 11))?), OBSTACLE_COST);
        assert_eq!(dists.get(RoomXY::try_from((10, 12))?), UNREACHABLE_COST);
        assert_eq!(dists.get(RoomXY::try_from((11, 12))?), OBSTACLE_COST);
        assert_eq!(dists.get(RoomXY::try_from((12, 12))?), UNREACHABLE_COST);
        Ok(())
    }
}
