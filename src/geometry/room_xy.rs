use crate::geometry::direction::OFFSET_BY_DIRECTION;
use crate::geometry::rect::{ball, Rect};
use crate::geometry::room_coordinate::RoomCoordinateUtils;
use enum_iterator::all;
use screeps::{
    Direction,
    OutOfBoundsError,
    Position,
    RoomCoordinate,
    RoomName,
    RoomXY,
    ROOM_SIZE
};
use std::cmp::{max, min};

pub trait RoomXYUtils
where
    Self: Sized,
{
    fn to_index(self) -> usize;
    unsafe fn rect_index(self, rect: Rect) -> usize;
    fn around(self) -> impl Iterator<Item = RoomXY>;
    fn outward_iter(self, min_r: Option<u8>, max_r: Option<u8>) -> impl Iterator<Item = RoomXY>;
    fn restricted_around(self, rect: Rect) -> impl Iterator<Item = RoomXY>;
    fn exit_distance(self) -> u8;
    fn max_exit_distance(self) -> u8;
    fn is_on_boundary(&self) -> bool;
    fn midpoint(self, other: Self) -> Self;

    fn sub(self, other: Self) -> (i8, i8);
    unsafe fn add_diff(self, diff: (i8, i8)) -> Self;
    fn try_add_diff(self, diff: (i8, i8)) -> Result<Self, OutOfBoundsError>;
    fn saturated_add_diff(self, diff: (i8, i8)) -> Self;

    fn direction_to(self, target: Self) -> Option<Direction>;

    fn dist(self, other: Self) -> u8;

    fn to_pos(self, room_name: RoomName) -> Position;
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
        all::<Direction>().filter_map(move |d| self.try_add_diff(OFFSET_BY_DIRECTION[d as usize]).ok())
    }

    #[inline]
    fn outward_iter(self, min_r: Option<u8>, max_r: Option<u8>) -> impl Iterator<Item = RoomXY> {
        let self_copy = self;
        (min_r.unwrap_or(0)..max_r.unwrap_or(self_copy.max_exit_distance()))
            .flat_map(move |r| ball(self_copy, r).boundary().filter(move |&xy| xy.dist(self_copy) == r))
    }

    fn restricted_around(self, rect: Rect) -> impl Iterator<Item = RoomXY> {
        // The function is safe because valid Rect consists of RoomCoordinate which are withing room bounds.
        all::<Direction>().filter_map(move |d| {
            let (dx, dy) = OFFSET_BY_DIRECTION[d as usize];
            let x = self.x.u8() as i8 + dx;
            let y = self.y.u8() as i8 + dy;
            if rect.contains_i8xy(x, y) {
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

    fn max_exit_distance(self) -> u8 {
        max(
            max(self.x.u8(), self.y.u8()),
            max(ROOM_SIZE - 1 - self.x.u8(), ROOM_SIZE - 1 - self.y.u8()),
        )
    }
    
    fn is_on_boundary(&self) -> bool {
        self.x.u8() == 0 || self.y.u8() == 0 || self.x.u8() == ROOM_SIZE - 1 || self.y.u8() == ROOM_SIZE - 1
    }

    fn midpoint(self, other: Self) -> Self {
        // Average of two points within room bounds is also within room bounds.
        unsafe { RoomXY::unchecked_new((self.x.u8() + other.x.u8()) / 2, (self.y.u8() + other.y.u8()) / 2) }
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

    fn saturated_add_diff(self, diff: (i8, i8)) -> Self {
        unsafe {
            RoomXY::from((
                RoomCoordinate::unchecked_new(max(0, min(ROOM_SIZE as i8 - 1, self.x.u8() as i8 + diff.0)) as u8),
                RoomCoordinate::unchecked_new(max(0, min(ROOM_SIZE as i8 - 1, self.y.u8() as i8 + diff.1)) as u8),
            ))
        }
    }

    fn direction_to(self, target: Self) -> Option<Direction> {
        // Logic copied from screeps-game-api and https://github.com/screeps/engine/blob/020ba168a1fde9a8072f9f1c329d5c0be8b440d7/src/utils.js#L73-L107
        let (dx, dy) = target.sub(self);
        if dx.abs() > dy.abs() * 2 {
            if dx > 0 {
                Some(Direction::Right)
            } else {
                Some(Direction::Left)
            }
        } else if dy.abs() > dx.abs() * 2 {
            if dy > 0 {
                Some(Direction::Bottom)
            } else {
                Some(Direction::Top)
            }
        } else if dx > 0 && dy > 0 {
            Some(Direction::BottomRight)
        } else if dx > 0 && dy < 0 {
            Some(Direction::TopRight)
        } else if dx < 0 && dy > 0 {
            Some(Direction::BottomLeft)
        } else if dx < 0 && dy < 0 {
            Some(Direction::TopLeft)
        } else {
            None
        }
    }

    fn dist(self, other: Self) -> u8 {
        max(
            (self.x.u8() as i8 - other.x.u8() as i8).abs(),
            (self.y.u8() as i8 - other.y.u8() as i8).abs(),
        ) as u8
    }

    fn to_pos(self, room_name: RoomName) -> Position {
        Position::new(self.x, self.y, room_name)
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
            assert_eq!(RoomXY::unchecked_new(ROOM_SIZE - 1, ROOM_SIZE - 1).exit_distance(), 0);
            assert_eq!(RoomXY::unchecked_new(ROOM_SIZE - 2, ROOM_SIZE - 3).exit_distance(), 1);
            assert_eq!(RoomXY::unchecked_new(10, 13).exit_distance(), 10);
        }
    }
}
