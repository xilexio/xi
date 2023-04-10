use crate::algorithms::matrix_common::MatrixCommon;
use crate::algorithms::room_matrix::RoomMatrix;
use crate::consts::OBSTACLE_COST;
use screeps::{RoomXY, ROOM_SIZE};
use std::cmp::{max, min};
use crate::geometry::rect::room_rect;

pub fn distance_transform_from_obstacles<O>(obstacles: O) -> RoomMatrix<u8>
where
    O: Iterator<Item = RoomXY>,
{
    let mut result = RoomMatrix::new(OBSTACLE_COST);
    for xy in room_rect().boundary() {
        result.set(xy, 1);
    }
    for xy in obstacles {
        result.set(xy, 0);
    }
    distance_transform(&mut result);
    result
}

pub fn l1_distance_transform_from_obstacles<O>(obstacles: O) -> RoomMatrix<u8>
    where
        O: Iterator<Item = RoomXY>,
{
    let mut result = RoomMatrix::new(OBSTACLE_COST);
    for xy in room_rect().boundary() {
        result.set(xy, 1);
    }
    for xy in obstacles {
        result.set(xy, 0);
    }
    horizontal_vertical_distance_transform(&mut result);
    result
}

/// Performs a distance transform. The matrix should have 0 on all obstacles and at least ROOM_SIZE
/// on other tiles.
pub fn distance_transform<T>(matrix: &mut T)
where
    T: MatrixCommon<u8>,
{
    horizontal_vertical_distance_transform(matrix);
    cross_distance_transform(matrix);
}

pub fn horizontal_vertical_distance_transform<T>(matrix: &mut T)
where
    T: MatrixCommon<u8>,
{
    let mut dist = ROOM_SIZE;
    for y in 0..ROOM_SIZE {
        // Towards right.
        for x in 0..ROOM_SIZE {
            unsafe {
                dist = min(matrix.get_xy(x, y), dist + 1);
                matrix.set_xy(x, y, dist);
            }
        }
        dist = ROOM_SIZE;
        // Towards left.
        for x in 0..ROOM_SIZE {
            unsafe {
                dist = min(matrix.get_xy(ROOM_SIZE - 1 - x, y), dist + 1);
                matrix.set_xy(ROOM_SIZE - 1 - x, y, dist);
            }
        }
    }
    for x in 0..ROOM_SIZE {
        dist = ROOM_SIZE;
        // Towards bottom.
        for y in 0..ROOM_SIZE {
            unsafe {
                dist = min(matrix.get_xy(x, y), dist + 1);
                matrix.set_xy(x, y, dist);
            }
        }
        dist = ROOM_SIZE;
        // Towards top.
        for y in 0..ROOM_SIZE {
            unsafe {
                dist = min(matrix.get_xy(x, ROOM_SIZE - 1 - y), dist + 1);
                matrix.set_xy(x, ROOM_SIZE - 1 - y, dist);
            }
        }
    }
}

pub fn cross_distance_transform<T>(matrix: &mut T)
where
    T: MatrixCommon<u8>,
{
    let size = ROOM_SIZE as i8;
    let mut dist = ROOM_SIZE;
    for y in 0..(2 * size - 1) {
        // Towards top right.
        for x in max(0, y - size + 1)..min(y + 1, size) {
            unsafe {
                dist = min(matrix.get_xy(x as u8, (y - x) as u8), dist + 1);
                matrix.set_xy(x as u8, (y - x) as u8, dist);
            }
        }
        dist = ROOM_SIZE;
        // Towards bottom left.
        for x in max(0, y - size + 1)..min(y + 1, size) {
            unsafe {
                dist = min(matrix.get_xy((size - 1 - x) as u8, (size - 1 - y + x) as u8), dist + 1);
                matrix.set_xy((size - 1 - x) as u8, (size - 1 - y + x) as u8, dist);
            }
        }
    }
    for y in 0..(2 * size - 1) {
        dist = ROOM_SIZE;
        // Towards bottom right.
        for x in max(0, y - size + 1)..min(y + 1, size) {
            unsafe {
                dist = min(matrix.get_xy(x as u8, (size - 1 - y + x) as u8), dist + 1);
                matrix.set_xy(x as u8, (size - 1 - y + x) as u8, dist);
            }
        }
        dist = ROOM_SIZE;
        // Towards top left.
        for x in max(0, y - size + 1)..min(y + 1, size) {
            unsafe {
                dist = min(matrix.get_xy((size - 1 - x) as u8, (y - x) as u8), dist + 1);
                matrix.set_xy((size - 1 - x) as u8, (y - x) as u8, dist);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::algorithms::distance_transform::{distance_transform, l1_distance_transform_from_obstacles};
    use crate::algorithms::matrix_common::MatrixCommon;
    use crate::algorithms::room_matrix::RoomMatrix;

    #[test]
    fn test_distance_transform_from_obstacles() {
        let mut matrix = RoomMatrix::new(0);
        unsafe {
            for y in 20..23 {
                for x in 10..13 {
                    matrix.set_xy(x, y, 255);
                }
            }
            matrix.set_xy(11, 19, 255);
            matrix.set_xy(9, 21, 255);
            matrix.set_xy(13, 21, 255);
            matrix.set_xy(11, 23, 255);
        }

        distance_transform(&mut matrix);

        unsafe {
            assert_eq!(matrix.get_xy(10, 19), 0);
            assert_eq!(matrix.get_xy(11, 19), 1);
            assert_eq!(matrix.get_xy(11, 20), 1);
            assert_eq!(matrix.get_xy(10, 20), 1);
            assert_eq!(matrix.get_xy(11, 21), 2);
        }
    }

    #[test]
    fn test_distance_transform() {
        let mut matrix = RoomMatrix::new(0);
        for y in 20..23 {
            for x in 10..13 {
                unsafe {
                    matrix.set_xy(x, y, 255);
                }
            }
        }

        distance_transform(&mut matrix);

        unsafe {
            assert_eq!(matrix.get_xy(0, 0), 0);
            assert_eq!(matrix.get_xy(30, 0), 0);
            assert_eq!(matrix.get_xy(15, 22), 0);
            assert_eq!(matrix.get_xy(10, 20), 1);
            assert_eq!(matrix.get_xy(11, 20), 1);
            assert_eq!(matrix.get_xy(12, 20), 1);
            assert_eq!(matrix.get_xy(10, 21), 1);
            assert_eq!(matrix.get_xy(11, 21), 2);
            assert_eq!(matrix.get_xy(12, 21), 1);
            assert_eq!(matrix.get_xy(10, 22), 1);
            assert_eq!(matrix.get_xy(11, 22), 1);
            assert_eq!(matrix.get_xy(12, 22), 1);
        }
    }

    #[test]
    fn test_l1_distance_transform_from_obstacles() {
        let obstacles = [
            (10, 10).try_into().unwrap(),
            (12, 12).try_into().unwrap(),
            (15, 12).try_into().unwrap(),
            (12, 13).try_into().unwrap(),
            (13, 13).try_into().unwrap(),
            (14, 13).try_into().unwrap(),
            (15, 13).try_into().unwrap(),
            (16, 13).try_into().unwrap(),
            (14, 14).try_into().unwrap(),
        ].into_iter();

        let dm_l1 = l1_distance_transform_from_obstacles(obstacles);

        unsafe {
            assert_eq!(dm_l1.get_xy(0, 0), 1);
            assert_eq!(dm_l1.get_xy(10, 10), 0);
            assert_eq!(dm_l1.get_xy(11, 10), 1);
            assert_eq!(dm_l1.get_xy(10, 11), 1);
            assert_eq!(dm_l1.get_xy(11, 11), 2);
            assert_eq!(dm_l1.get_xy(12, 11), 1);
            assert_eq!(dm_l1.get_xy(13, 11), 2);
            assert_eq!(dm_l1.get_xy(13, 12), 1);
            assert_eq!(dm_l1.get_xy(15, 14), 1);
            assert_eq!(dm_l1.get_xy(15, 15), 2);
        }
    }
}
