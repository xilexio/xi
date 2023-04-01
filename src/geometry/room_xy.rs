use std::cmp::{max, min};
use crate::geometry::direction::offset_from_direction;
use crate::geometry::rect::Rect;
use crate::geometry::room_coordinate::RoomCoordinateUtils;
use enum_iterator::IntoEnumIterator;
use screeps::{Direction, RoomXY, ROOM_SIZE};

pub trait RoomXYUtils {
    fn to_index(&self) -> usize;
    unsafe fn rect_index(&self, rect: Rect) -> usize;
    fn around(&self) -> Vec<RoomXY>;
    unsafe fn restricted_around(&self, rect: Rect) -> Vec<RoomXY>;
    fn exit_distance(&self) -> u8;

    fn sub(self, other: Self) -> (i8, i8);
    unsafe fn add_diff(self, diff: (i8, i8)) -> Self;

    fn cdist(self, other: Self) -> u8;
}

impl RoomXYUtils for RoomXY {
    #[inline]
    fn to_index(&self) -> usize {
        (self.x.u8() as usize) + (ROOM_SIZE as usize) * (self.y.u8() as usize)
    }

    unsafe fn rect_index(&self, rect: Rect) -> usize {
        let (dx, dy) = self.sub(rect.top_left);
        (dx as usize) + (rect.width() as usize) * (dy as usize)
    }

    // TODO to iter and js benchmark of this and table-based
    #[inline]
    fn around(&self) -> Vec<RoomXY> {
        let mut result = Vec::new();
        let (x, y) = (self.x.u8() as i8, self.y.u8() as i8);
        for near_y in (y - 1)..(y + 2) {
            for near_x in (x - 1)..(x + 2) {
                if (near_y != y || near_x != x) && 0 <= near_x
                        && (near_x as u8) < ROOM_SIZE
                        && 0 <= near_y && (near_y as u8) < ROOM_SIZE {
                    // (near_x, near_y) was already checked to be within bounds, so it is safe.
                    result.push(unsafe { RoomXY::unchecked_new(near_x as u8, near_y as u8) });
                }
            }
        }
        result
    }

    // The function is safe if and only if the rect is within room bounds.
    unsafe fn restricted_around(&self, rect: Rect) -> Vec<RoomXY> {
        let mut result = Vec::new();
        for d in Direction::into_enum_iter() {
            let near = self.add_diff(offset_from_direction(d));
            if rect.is_inside(near) {
                result.push(near);
            }
        }
        result
    }

    #[inline]
    fn exit_distance(&self) -> u8 {
        min(
            min(self.x.u8(), self.y.u8()),
            min(ROOM_SIZE - 1 - self.x.u8(), ROOM_SIZE - 1 - self.y.u8())
        )
    }

    fn sub(self, other: Self) -> (i8, i8) {
        (self.x.sub(other.x), self.y.sub(other.y))
    }

    unsafe fn add_diff(self, diff: (i8, i8)) -> Self {
        (self.x.add_diff(diff.0), self.y.add_diff(diff.1)).into()
    }

    fn cdist(self, other: Self) -> u8 {
        max((self.x.u8() as i8 - other.x.u8() as i8).abs(), (self.y.u8() as i8 - other.y.u8() as i8).abs()) as u8
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
                xy.around(),
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
                xy.around(),
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
                xy.around(),
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
                xy.around(),
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
            assert_eq!(RoomXY::unchecked_new(ROOM_SIZE - 1, ROOM_SIZE - 1).exit_distance(), 0);
            assert_eq!(RoomXY::unchecked_new(ROOM_SIZE - 2, ROOM_SIZE - 3).exit_distance(), 1);
            assert_eq!(RoomXY::unchecked_new(10, 13).exit_distance(), 10);
        }
    }
}
