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

    fn sub_diff(self, other: Self) -> (i8, i8);
    unsafe fn sub(self, other: Self) -> Self;
    unsafe fn add_diff(self, diff: (i8, i8)) -> Self;
}

impl RoomXYUtils for RoomXY {
    fn to_index(&self) -> usize {
        (self.x.u8() as usize) + (ROOM_SIZE as usize) * (self.y.u8() as usize)
    }

    unsafe fn rect_index(&self, rect: Rect) -> usize {
        let relative_xy = self.sub(rect.top_left);
        (relative_xy.x.u8() as usize) + (rect.width() as usize) * (relative_xy.y.u8() as usize)
    }

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
        // for d in Direction::into_enum_iter() {
        //     let (dx, dy) = offset_from_direction(d);
        //     let (near_x, near_y) = (x + dx, y + dy);
        //     if 0 <= near_x
        //         && (near_x as u8) < ROOM_SIZE
        //         && 0 <= near_y
        //         && (near_y as u8) < ROOM_SIZE
        //     {
        //         // (near_x, near_y) was already checked to be within bounds, so it is safe.
        //         result.push(unsafe { RoomXY::unchecked_new(near_x as u8, near_y as u8) });
        //     }
        // }
        result
    }

    // The function is safe if and only if the rect is within room bounds.
    unsafe fn restricted_around(&self, rect: Rect) -> Vec<RoomXY> {
        let mut result = Vec::new();
        for d in Direction::into_enum_iter() {
            let near = self.add_diff(offset_from_direction(d));
            if rect.inside(near) {
                result.push(near);
            }
        }
        result
    }

    fn sub_diff(self, other: Self) -> (i8, i8) {
        (self.x.sub_diff(other.x), self.y.sub_diff(other.y))
    }

    unsafe fn sub(self, other: Self) -> Self {
        (self.x.sub(other.x), self.y.sub(other.y)).into()
    }

    unsafe fn add_diff(self, diff: (i8, i8)) -> Self {
        (self.x.add_diff(diff.0), self.y.add_diff(diff.1)).into()
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
}
