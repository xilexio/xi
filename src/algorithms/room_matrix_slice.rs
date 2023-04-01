use crate::algorithms::matrix_common::MatrixCommon;
use crate::consts::ROOM_AREA;
use crate::geometry::rect::Rect;
use crate::geometry::room_xy::RoomXYUtils;
use screeps::{RoomXY, ROOM_SIZE};

pub struct RoomMatrixSlice<T> {
    pub rect: Rect,
    pub data: Vec<T>,
}

impl<T> RoomMatrixSlice<T>
where
    T: Default + num_traits::PrimInt,
{
    pub fn new(rect: Rect) -> Self {
        RoomMatrixSlice::new_custom_filled(rect, T::default())
    }

    pub fn new_custom_filled(rect: Rect, fill: T) -> Self {
        let mut data = Vec::new();
        data.resize_with(rect.area(), || fill);
        RoomMatrixSlice { rect, data }
    }
}

impl<T> MatrixCommon<T> for RoomMatrixSlice<T>
where
    T: num_traits::PrimInt + 'static,
{
    #[inline]
    fn get(&self, xy: RoomXY) -> T {
        unsafe { self.data[xy.rect_index(self.rect)] }
    }

    #[inline]
    fn set(&mut self, xy: RoomXY, value: T) {
        unsafe {
            self.data[xy.rect_index(self.rect)] = value;
        }
    }

    fn iter(&self) -> impl Iterator<Item = (RoomXY, T)> + '_ {
        let base_x = self.rect.top_left.x.u8();
        let base_y = self.rect.top_left.y.u8();
        let width = self.rect.width() as u16;
        let height = self.rect.height() as u16;
        (0..(width * height)).map(move |i| {
            let xy = unsafe {
                RoomXY::unchecked_new(base_x + (i % width) as u8, base_y + (i / width) as u8)
            };
            (xy, self.get(xy))
        })
    }
}
