use screeps::RoomXY;
use crate::algorithms::matrix_common::MatrixCommon;
use crate::geometry::rect::Rect;
use crate::geometry::room_xy::RoomXYUtils;

pub struct RoomMatrixSlice<T> {
    pub rect: Rect,
    pub data: Vec<T>,
}

impl<T> RoomMatrixSlice<T>
where
    T: Default + Copy,
{
    pub fn new(rect: Rect) -> Self {
        RoomMatrixSlice::new_custom_filled(rect, T::default())
    }

    pub fn new_custom_filled(rect: Rect, fill: T) -> Self {
        let mut data = Vec::new();
        data.resize_with(rect.area(), || fill);
        RoomMatrixSlice {
            rect,
            data,
        }
    }
}

impl<T> MatrixCommon<T> for RoomMatrixSlice<T>
    where
        T: Default + Copy,
{
    fn get(&self, xy: RoomXY) -> T {
        unsafe {
            self.data[xy.rect_index(self.rect)]
        }
    }

    fn set(&mut self, xy: RoomXY, value: T) {
        unsafe {
            self.data[xy.rect_index(self.rect)] = value;
        }
    }
}
