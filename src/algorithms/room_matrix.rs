use crate::algorithms::matrix_common::MatrixCommon;
use crate::consts::ROOM_AREA;
use screeps::{RoomXY};
use crate::geometry::room_xy::RoomXYUtils;

/// A `ROOM_SIZE` x `ROOM_SIZE` matrix backed by an array with size known at compile time.
pub struct RoomMatrix<T> {
    pub data: [T; ROOM_AREA],
}

impl<T> RoomMatrix<T>
where
    T: Default + Copy,
{
    pub fn new() -> Self {
        RoomMatrix::new_custom_filled(T::default())
    }

    pub fn new_custom_filled(fill: T) -> Self {
        RoomMatrix {
            data: [fill; ROOM_AREA],
        }
    }
}

impl<T> MatrixCommon<T> for RoomMatrix<T>
where
    T: Default + Copy,
{
    fn get(&self, xy: RoomXY) -> T {
        self.data[xy.to_index()]
    }

    fn set(&mut self, xy: RoomXY, value: T) {
        self.data[xy.to_index()] = value;
    }
}
