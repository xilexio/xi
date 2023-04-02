use std::cmp::{max, min};
use screeps::ROOM_SIZE;
use crate::algorithms::matrix_common::MatrixCommon;

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
        for x in 0..ROOM_SIZE {
            unsafe {
                dist = min(matrix.get_xy(x, y), dist + 1);
                matrix.set_xy(x, y, dist);
            }
        }
        dist = ROOM_SIZE;
        for x in 0..ROOM_SIZE {
            unsafe {
                dist = min(matrix.get_xy(ROOM_SIZE - 1 - x, y), dist + 1);
                matrix.set_xy(ROOM_SIZE - 1 - x, y, dist);
            }
        }
        dist = ROOM_SIZE;
        for x in 0..ROOM_SIZE {
            unsafe {
                dist = min(matrix.get_xy(y, x), dist + 1);
                matrix.set_xy(y, x, dist);
            }
        }
        dist = ROOM_SIZE;
        for x in 0..ROOM_SIZE {
            unsafe {
                dist = min(matrix.get_xy(ROOM_SIZE - 1 - y, x), dist + 1);
                matrix.set_xy(ROOM_SIZE - 1 - y, x, dist);
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
    for y in 0i8..(2 * size - 1) {
        for x in max(0, y - size + 1)..min(y + 1, size) {
            unsafe {
                dist = min(matrix.get_xy(x as u8, (y - x) as u8), dist + 1);
                matrix.set_xy(x as u8, (y - x) as u8, dist);
            }
        }
        dist = ROOM_SIZE;
        for x in max(0, y - size + 1)..min(y + 1, size) {
            unsafe {
                dist = min(matrix.get_xy(x as u8, (size - 1 - y + x) as u8), dist + 1);
                matrix.set_xy(x as u8, (size - 1 - y + x) as u8, dist);
            }
        }
        dist = ROOM_SIZE;
        for x in max(0, y - size + 1)..min(y + 1, size) {
            unsafe {
                dist = min(matrix.get_xy((size - 1 - x) as u8, (y - x) as u8), dist + 1);
                matrix.set_xy((size - 1 - x) as u8, (y - x) as u8, dist);
            }
        }
        dist = ROOM_SIZE;
        for x in max(0, y - size + 1)..min(y + 1, size) {
            unsafe {
                dist = min(matrix.get_xy((size - 1 - x) as u8, (size - 1 - y + x) as u8), dist + 1);
                matrix.set_xy((size - 1 - x) as u8, (size - 1 - y + x) as u8, dist);
            }
        }
    }
}

#[cfg(test)]
mod test {
    use crate::algorithms::distance_transform::distance_transform;
    use crate::algorithms::matrix_common::MatrixCommon;
    use crate::algorithms::room_matrix::RoomMatrix;

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
}