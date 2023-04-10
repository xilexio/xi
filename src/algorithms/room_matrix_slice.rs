use crate::algorithms::matrix_common::MatrixCommon;
use crate::geometry::rect::Rect;
use crate::geometry::room_xy::RoomXYUtils;
use screeps::RoomXY;
use std::error::Error;

#[derive(Clone)]
pub struct RoomMatrixSlice<T> {
    pub rect: Rect,
    pub data: Vec<T>,
}

impl<T> RoomMatrixSlice<T>
where
    T: Clone + Copy + PartialEq,
{
    pub fn new(rect: Rect, fill: T) -> Self {
        let mut data = Vec::new();
        data.resize_with(rect.area(), || fill);
        RoomMatrixSlice { rect, data }
    }

    pub fn translate(&mut self, offset: (i8, i8)) -> Result<(), Box<dyn Error>> {
        let top_left = self.rect.top_left.try_add_diff(offset)?;
        let bottom_right = self.rect.bottom_right.try_add_diff(offset)?;
        self.rect.top_left = top_left;
        self.rect.bottom_right = bottom_right;
        Ok(())
    }

    /// Rotates the slice clockwise `rotations` times.
    pub fn rotate(&mut self, rotations: u8) -> Result<(), Box<dyn Error>> {
        let w = self.rect.width();
        let h = self.rect.height();
        let r = rotations % 4;
        if r == 0 {
            return Ok(());
        }

        if w == h {
            let x0 = self.rect.top_left.x.u8();
            let y0 = self.rect.top_left.y.u8();
            for y in 0..(h / 2) {
                for x in 0..((w + 1) / 2) {
                    let xys = unsafe {
                        [
                            RoomXY::unchecked_new(x0 + x, y0 + y),
                            RoomXY::unchecked_new(x0 + h - 1 - y, y0 + x),
                            RoomXY::unchecked_new(x0 + w - 1 - x, y0 + h - 1 - y),
                            RoomXY::unchecked_new(x0 + y, y0 + w - 1 - x),
                        ]
                    };
                    let vals = [self.get(xys[0]), self.get(xys[1]), self.get(xys[2]), self.get(xys[3])];
                    self.set(xys[r as usize], vals[0]);
                    self.set(xys[((r + 1) % 4) as usize], vals[1]);
                    self.set(xys[((r + 2) % 4) as usize], vals[2]);
                    self.set(xys[((r + 3) % 4) as usize], vals[3]);
                }
            }
            Ok(())
        } else {
            todo!("rotation of non-square")
        }
    }
}

impl<T> MatrixCommon<T> for RoomMatrixSlice<T>
where
    T: Clone + Copy + PartialEq,
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

    fn iter_xy<'a, 'b>(&'a self) -> impl Iterator<Item = RoomXY> + 'b {
        let base_x = self.rect.top_left.x.u8();
        let base_y = self.rect.top_left.y.u8();
        let width = self.rect.width() as u16;
        let height = self.rect.height() as u16;
        (0..(width * height))
            .map(move |i| unsafe { RoomXY::unchecked_new(base_x + (i % width) as u8, base_y + (i / width) as u8) })
    }
}

#[cfg(test)]
mod tests {
    use crate::algorithms::matrix_common::MatrixCommon;
    use crate::algorithms::room_matrix_slice::RoomMatrixSlice;
    use crate::geometry::rect::Rect;

    #[test]
    fn test_rotation() {
        let mut slice = RoomMatrixSlice::new(
            Rect::new((2, 1).try_into().unwrap(), (6, 5).try_into().unwrap()).unwrap(),
            0,
        );
        let mut i = 0;
        slice.update(|xy, v| {
            i += 1;
            i
        });
        assert_eq!(slice.get((2, 2).try_into().unwrap()), 6);
        assert_eq!(slice.get((4, 3).try_into().unwrap()), 13);
        slice.rotate(1).unwrap();
        assert_eq!(slice.get((2, 2).try_into().unwrap()), 22);
        assert_eq!(slice.get((4, 3).try_into().unwrap()), 13);
        slice.rotate(2).unwrap();
        assert_eq!(slice.get((2, 2).try_into().unwrap()), 4);
        assert_eq!(slice.get((4, 3).try_into().unwrap()), 13);
        slice.rotate(3).unwrap();
        assert_eq!(slice.get((2, 2).try_into().unwrap()), 20);
        assert_eq!(slice.get((4, 3).try_into().unwrap()), 13);
    }
}
