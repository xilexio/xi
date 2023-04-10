use crate::algorithms::matrix_common::MatrixCommon;
use crate::algorithms::room_matrix::RoomMatrix;
use crate::algorithms::room_matrix_slice::RoomMatrixSlice;
use crate::consts::{OBSTACLE_COST, UNREACHABLE_COST};
use crate::geometry::rect::Rect;
use crate::geometry::room_xy::RoomXYUtils;
use screeps::RoomXY;
use std::cmp::min;

/// Computes a matrix with distances from all tiles in the room to given target. OBSTACLE_COST where there are
/// obstacles, UNREACHABLE_COST when the target is unreachable and the distance is clamped at UNREACHABLE_COST.
pub fn distance_matrix<T, O>(obstacles: O, target: T) -> RoomMatrix<u8>
where
    T: Iterator<Item = RoomXY>,
    O: Iterator<Item = RoomXY>,
{
    let mut result = RoomMatrix::new(UNREACHABLE_COST);

    for xy in obstacles {
        result.set(xy, OBSTACLE_COST);
    }

    let mut layer = Vec::new();

    for xy in target {
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

pub fn rect_restricted_distance_matrix<O, T>(
    obstacles: O,
    target: T,
    slice: Rect,
    max_distance: u8,
) -> RoomMatrixSlice<u8>
where
    O: Iterator<Item = RoomXY>,
    T: Iterator<Item = RoomXY>,
{
    let mut result = RoomMatrixSlice::new(slice, UNREACHABLE_COST);

    for xy in obstacles {
        result.set(xy, OBSTACLE_COST);
    }

    let mut layer = Vec::new();

    for xy in target {
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

pub fn count_restricted_distance_matrix<O>(obstacles: O, target: RoomXY, max_tiles: u16) -> RoomMatrix<u8>
where
    O: Iterator<Item = RoomXY>,
{
    let mut result = RoomMatrix::new(UNREACHABLE_COST);

    for xy in obstacles {
        result.set(xy, OBSTACLE_COST);
    }

    let mut layer = Vec::new();

    result.set(target, 0);
    layer.push(target);

    let mut distance = 1u8;
    let mut tiles_count = 1u16;

    'main_loop: while !layer.is_empty() {
        let mut next_layer = Vec::new();
        for xy in layer {
            for near in xy.around() {
                if result.get(near) == UNREACHABLE_COST {
                    result.set(near, distance);
                    next_layer.push(near);
                    tiles_count += 1;
                    if tiles_count >= max_tiles {
                        break 'main_loop;
                    }
                }
            }
        }
        layer = next_layer;
        distance = min(UNREACHABLE_COST - 1, distance + 1);
    }

    result
}

pub fn targeted_distance_matrix<O, S, T>(obstacles: O, start: S, targets: T) -> Option<RoomMatrix<u8>>
where
    O: Iterator<Item = RoomXY>,
    S: Iterator<Item = RoomXY>,
    T: Iterator<Item = RoomXY>,
{
    let mut result = RoomMatrix::new(UNREACHABLE_COST);
    let mut targets_left = 0u8;
    for xy in obstacles {
        result.set(xy, OBSTACLE_COST);
    }
    for xy in targets {
        if result.get(xy) == OBSTACLE_COST || result.get(xy) == UNREACHABLE_COST - 2 {
            result.set(xy, UNREACHABLE_COST - 2);
        } else {
            result.set(xy, UNREACHABLE_COST - 1);
        }
        targets_left += 1;
    }

    let mut layer = Vec::new();

    for xy in start {
        if result.get(xy) == UNREACHABLE_COST - 2 || result.get(xy) == UNREACHABLE_COST - 1 {
            targets_left -= 1;
        }
        result.set(xy, 0);
        layer.push(xy);
    }

    let mut distance = 1u8;

    while !layer.is_empty() && distance < UNREACHABLE_COST - 2 {
        let mut next_layer = Vec::new();
        for xy in layer {
            for near in xy.around() {
                let near_value = result.get(near);
                if near_value == UNREACHABLE_COST || near_value == UNREACHABLE_COST - 1 {
                    result.set(near, distance);
                    next_layer.push(near);
                    if near_value == UNREACHABLE_COST - 1 {
                        targets_left -= 1;
                        if targets_left == 0 {
                            return Some(result);
                        }
                    }
                } else if near_value == UNREACHABLE_COST - 2 {
                    result.set(near, OBSTACLE_COST);
                    targets_left -= 1;
                    if targets_left == 0 {
                        return Some(result);
                    }
                }
            }
        }

        layer = next_layer;
        distance += 1;
    }

    None
}

#[cfg(test)]
mod tests {
    use crate::algorithms::distance_matrix::{
        distance_matrix, targeted_distance_matrix, rect_restricted_distance_matrix,
    };
    use crate::algorithms::matrix_common::MatrixCommon;
    use crate::consts::{OBSTACLE_COST, ROOM_AREA, UNREACHABLE_COST};
    use crate::geometry::rect::Rect;
    use more_asserts::assert_ge;
    use screeps::RoomXY;
    use std::error::Error;
    use std::iter::once;

    #[test]
    fn test_restricted_grid_bfs_distances() -> Result<(), Box<dyn Error>> {
        let slice = Rect::new(RoomXY::try_from((10, 10))?, RoomXY::try_from((12, 12))?)?;
        let dists = rect_restricted_distance_matrix(
            [
                RoomXY::try_from((11, 11))?,
                RoomXY::try_from((12, 11))?,
                RoomXY::try_from((11, 12))?,
            ]
            .into_iter(),
            once(RoomXY::try_from((10, 10))?),
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
        let dists = rect_restricted_distance_matrix(
            [
                RoomXY::try_from((11, 11))?,
                RoomXY::try_from((12, 11))?,
                RoomXY::try_from((11, 12))?,
            ]
            .into_iter(),
            [RoomXY::try_from((10, 10))?].into_iter(),
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

    #[test]
    fn test_distance_matrix_starting_from_an_obstacle() {
        let dm = distance_matrix(
            [
                (24, 25).try_into().unwrap(),
                (25, 25).try_into().unwrap(),
                (26, 25).try_into().unwrap(),
            ]
            .into_iter(),
            once((25, 25).try_into().unwrap()),
        );

        assert_eq!(dm.get((25, 25).try_into().unwrap()), 0);
        assert_eq!(dm.get((24, 25).try_into().unwrap()), OBSTACLE_COST);
        assert_eq!(dm.get((24, 24).try_into().unwrap()), 1);
        assert_eq!(dm.get((24, 26).try_into().unwrap()), 1);
        assert_eq!(dm.get((23, 25).try_into().unwrap()), 2);
    }

    #[test]
    fn test_targeted_distance_matrix() {
        let dm = targeted_distance_matrix(
            [
                (24, 25).try_into().unwrap(),
                (25, 25).try_into().unwrap(),
                (26, 25).try_into().unwrap(),
            ]
            .into_iter(),
            once((25, 25).try_into().unwrap()),
            [
                (23, 25).try_into().unwrap(),
                (25, 20).try_into().unwrap(),
                (26, 26).try_into().unwrap(),
            ]
            .into_iter(),
        )
        .unwrap();

        assert_eq!(dm.get((25, 25).try_into().unwrap()), 0);
        assert_eq!(dm.get((24, 25).try_into().unwrap()), OBSTACLE_COST);
        assert_eq!(dm.get((24, 24).try_into().unwrap()), 1);
        assert_eq!(dm.get((26, 26).try_into().unwrap()), 1);
        assert_eq!(dm.get((25, 20).try_into().unwrap()), 5);
        assert_eq!(dm.get((25, 19).try_into().unwrap()), UNREACHABLE_COST);
        assert_ge!(
            dm.find_xy(OBSTACLE_COST).count() + dm.find_xy(UNREACHABLE_COST).count(),
            ROOM_AREA - 11 * 11 + 2
        );
    }

    #[test]
    fn test_targeted_distance_matrix_to_obstacle() {
        let dm = targeted_distance_matrix(
            [
                (24, 25).try_into().unwrap(),
                (25, 25).try_into().unwrap(),
                (26, 25).try_into().unwrap(),
                (26, 20).try_into().unwrap(),
                (26, 27).try_into().unwrap(),
            ]
            .into_iter(),
            [(24, 27).try_into().unwrap(), (26, 27).try_into().unwrap()].into_iter(),
            [
                (24, 20).try_into().unwrap(),
                (26, 20).try_into().unwrap(),
            ]
            .into_iter(),
        )
        .unwrap();

        assert_eq!(dm.get((24, 27).try_into().unwrap()), 0);
        assert_eq!(dm.get((26, 27).try_into().unwrap()), 0);
        assert_eq!(dm.get((24, 25).try_into().unwrap()), OBSTACLE_COST);
        assert_eq!(dm.get((25, 19).try_into().unwrap()), UNREACHABLE_COST);
    }
}
