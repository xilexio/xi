use crate::geometry::direction::{OFFSET_BY_DIRECTION};
use crate::geometry::rect::Rect;
use crate::geometry::room_coordinate::RoomCoordinateUtils;
use enum_iterator::IntoEnumIterator;
use screeps::{Direction, OutOfBoundsError, RoomXY, ROOM_SIZE};
use std::cmp::{max, min};

pub trait RoomXYUtils
where
    Self: Sized,
{
    fn to_index(self) -> usize;
    unsafe fn rect_index(self, rect: Rect) -> usize;
    fn around(self) -> impl Iterator<Item = RoomXY>;
    fn restricted_around(self, rect: Rect) -> impl Iterator<Item = RoomXY>;
    fn exit_distance(self) -> u8;

    fn sub(self, other: Self) -> (i8, i8);
    unsafe fn add_diff(self, diff: (i8, i8)) -> Self;
    fn try_add_diff(self, diff: (i8, i8)) -> Result<Self, OutOfBoundsError>;

    fn dist(self, other: Self) -> u8;
}

impl RoomXYUtils for RoomXY {
    #[inline]
    fn to_index(self) -> usize {
        (self.x.u8() as usize) + (ROOM_SIZE as usize) * (self.y.u8() as usize)
    }

    unsafe fn rect_index(self, rect: Rect) -> usize {
        let (dx, dy) = self.sub(rect.top_left);
        (dx as usize) + (rect.width() as usize) * (dy as usize)
    }

    /// Returns an iterator to all tiles neighboring the given tile, if they are within the room
    /// bounds.
    // 0.63ms per distance matrix with double for implementation
    // 0.44ms with direction iter + offset_from_direction implementation
    // 0.44ms with direction item + OFFSET_BY_DIRECTION implementation
    #[inline]
    fn around(self) -> impl Iterator<Item = RoomXY> {
        // let mut result = Vec::new();
        // let (x, y) = (self.x.u8() as i8, self.y.u8() as i8);
        // for near_y in (y - 1)..(y + 2) {
        //     for near_x in (x - 1)..(x + 2) {
        //         if (near_y != y || near_x != x) && 0 <= near_x
        //                 && (near_x as u8) < ROOM_SIZE
        //                 && 0 <= near_y && (near_y as u8) < ROOM_SIZE {
        //             // (near_x, near_y) was already checked to be within bounds, so it is safe.
        //             result.push(unsafe { RoomXY::unchecked_new(near_x as u8, near_y as u8) });
        //         }
        //     }
        // }
        // result.into_iter()
        // Direction::into_enum_iter().filter_map(|d| self.try_add_diff(offset_from_direction(d)).ok())
        Direction::into_enum_iter().filter_map(move |d| self.try_add_diff(OFFSET_BY_DIRECTION[d as usize]).ok())
    }

    fn restricted_around(self, rect: Rect) -> impl Iterator<Item = RoomXY> {
        // The function is safe because valid Rect consists of RoomCoordinate which are withing room bounds.
        Direction::into_enum_iter().filter_map(move |d| {
            let (dx, dy) = OFFSET_BY_DIRECTION[d as usize];
            let x = self.x.u8() as i8 + dx;
            let y = self.y.u8() as i8 + dy;
            if rect.is_i8xy_inside(x, y) {
                Some((x as u8, y as u8).try_into().unwrap())
            } else {
                None
            }
        })
    }

    #[inline]
    fn exit_distance(self) -> u8 {
        min(
            min(self.x.u8(), self.y.u8()),
            min(ROOM_SIZE - 1 - self.x.u8(), ROOM_SIZE - 1 - self.y.u8()),
        )
    }

    fn sub(self, other: Self) -> (i8, i8) {
        (self.x.sub(other.x), self.y.sub(other.y))
    }

    unsafe fn add_diff(self, diff: (i8, i8)) -> Self {
        (self.x.add_diff(diff.0), self.y.add_diff(diff.1)).into()
    }

    fn try_add_diff(self, diff: (i8, i8)) -> Result<Self, OutOfBoundsError> {
        Ok((self.x.try_add_diff(diff.0)?, self.y.try_add_diff(diff.1)?).into())
    }

    fn dist(self, other: Self) -> u8 {
        max(
            (self.x.u8() as i8 - other.x.u8() as i8).abs(),
            (self.y.u8() as i8 - other.y.u8() as i8).abs(),
        ) as u8
    }
}

#[cfg(test)]
mod tests {
    use crate::geometry::room_xy::RoomXYUtils;
    use screeps::{RoomXY, ROOM_SIZE};

    #[test]
    fn test_around_2_2() {
        unsafe {
            let xy = RoomXY::unchecked_new(2, 2);
            assert_eq!(
                xy.around().collect::<Vec<RoomXY>>(),
                vec![
                    RoomXY::unchecked_new(2, 1),
                    RoomXY::unchecked_new(3, 1),
                    RoomXY::unchecked_new(3, 2),
                    RoomXY::unchecked_new(3, 3),
                    RoomXY::unchecked_new(2, 3),
                    RoomXY::unchecked_new(1, 3),
                    RoomXY::unchecked_new(1, 2),
                    RoomXY::unchecked_new(1, 1),
                ]
            );
        }
    }

    #[test]
    fn test_around_0_0() {
        unsafe {
            let xy = RoomXY::unchecked_new(0, 0);
            assert_eq!(
                xy.around().collect::<Vec<RoomXY>>(),
                vec![
                    RoomXY::unchecked_new(1, 0),
                    RoomXY::unchecked_new(1, 1),
                    RoomXY::unchecked_new(0, 1),
                ]
            );
        }
    }

    #[test]
    fn test_around_49_49() {
        unsafe {
            let xy = RoomXY::unchecked_new(ROOM_SIZE - 1, ROOM_SIZE - 1);
            assert_eq!(
                xy.around().collect::<Vec<RoomXY>>(),
                vec![
                    RoomXY::unchecked_new(ROOM_SIZE - 1, ROOM_SIZE - 2),
                    RoomXY::unchecked_new(ROOM_SIZE - 2, ROOM_SIZE - 1),
                    RoomXY::unchecked_new(ROOM_SIZE - 2, ROOM_SIZE - 2),
                ]
            );
        }
    }

    #[test]
    fn test_around_0_42() {
        unsafe {
            let xy = RoomXY::unchecked_new(0, 42);
            assert_eq!(
                xy.around().collect::<Vec<RoomXY>>(),
                vec![
                    RoomXY::unchecked_new(0, 41),
                    RoomXY::unchecked_new(1, 41),
                    RoomXY::unchecked_new(1, 42),
                    RoomXY::unchecked_new(1, 43),
                    RoomXY::unchecked_new(0, 43),
                ]
            );
        }
    }

    #[test]
    fn test_exit_distance() {
        unsafe {
            assert_eq!(RoomXY::unchecked_new(0, 0).exit_distance(), 0);
            assert_eq!(RoomXY::unchecked_new(2, 4).exit_distance(), 2);
            assert_eq!(
                RoomXY::unchecked_new(ROOM_SIZE - 1, ROOM_SIZE - 1).exit_distance(),
                0
            );
            assert_eq!(
                RoomXY::unchecked_new(ROOM_SIZE - 2, ROOM_SIZE - 3).exit_distance(),
                1
            );
            assert_eq!(RoomXY::unchecked_new(10, 13).exit_distance(), 10);
        }
    }
}
