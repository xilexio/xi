use std::cmp::min;
use screeps::{RoomXY, ROOM_SIZE};
use std::error::Error;
use std::fmt::{Display, Formatter};
use crate::geometry::room_coordinate::RoomCoordinateUtils;

#[derive(Copy, Clone, Debug)]
pub struct Rect {
    pub top_left: RoomXY,
    pub bottom_right: RoomXY,
}

#[derive(Debug, Clone)]
pub struct InvalidRectError;

impl Display for InvalidRectError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Rect does not have a positive area")
    }
}

impl Error for InvalidRectError {}

impl Rect {
    pub fn new(top_left: RoomXY, bottom_right: RoomXY) -> Result<Self, InvalidRectError> {
        let result = Rect {
            top_left,
            bottom_right,
        };
        if result.is_valid() {
            Ok(result)
        } else {
            Err(InvalidRectError)
        }
    }

    pub unsafe fn unchecked_new(top_left: RoomXY, bottom_right: RoomXY) -> Self {
        Rect {
            top_left,
            bottom_right,
        }
    }

    pub fn is_valid(self) -> bool {
        self.top_left.x <= self.bottom_right.x && self.top_left.y <= self.bottom_right.y
    }

    pub fn width(self) -> u8 {
        self.bottom_right.x.u8() - self.top_left.x.u8() + 1
    }

    pub fn height(self) -> u8 {
        self.bottom_right.y.u8() - self.top_left.y.u8() + 1
    }

    pub fn area(self) -> usize {
        (self.width() as usize) * (self.height() as usize)
    }

    pub fn contains(self, xy: RoomXY) -> bool {
        self.top_left.x <= xy.x
            && xy.x <= self.bottom_right.x
            && self.top_left.y <= xy.y
            && xy.y <= self.bottom_right.y
    }

    pub fn contains_i8xy(self, x: i8, y: i8) -> bool {
        self.top_left.x.u8() as i8 <= x
            && x <= self.bottom_right.x.u8() as i8
            && self.top_left.y.u8() as i8 <= y
            && y <= self.bottom_right.y.u8() as i8
    }

    pub fn boundary(self) -> impl Iterator<Item = RoomXY> {
        unsafe {
            let top = (1..self.width()).map(move |dx| (self.top_left.x.add_diff(dx as i8), self.top_left.y).into());
            let right = (1..self.height()).map(move |dy| (self.bottom_right.x, self.top_left.y.add_diff(dy as i8)).into());
            let bottom = (1..self.width()).map(move |dx| (self.bottom_right.x.add_diff(-(dx as i8)), self.bottom_right.y).into());
            let left = (1..self.height()).map(move |dy| (self.top_left.x, self.bottom_right.y.add_diff(-(dy as i8))).into());

            top.chain(right).chain(bottom).chain(left)
        }
    }

    pub fn iter(self) -> impl Iterator<Item = RoomXY> {
        let tlx = self.top_left.x.u8();
        let tly = self.top_left.y.u8();
        let w = self.width() as u16;
        let h = self.height() as u16;
        (0..(w * h)).map(move |i| unsafe {
            RoomXY::unchecked_new(tlx + ((i % w) as u8), tly + ((i / w) as u8))
        })
    }
}

impl TryFrom<(RoomXY, RoomXY)> for Rect {
    type Error = InvalidRectError;

    fn try_from(xy_pair: (RoomXY, RoomXY)) -> Result<Self, Self::Error> {
        Rect::new(xy_pair.0, xy_pair.1)
    }
}

pub fn room_rect() -> Rect {
    unsafe {
        Rect::unchecked_new(
            RoomXY::unchecked_new(0, 0),
            RoomXY::unchecked_new(ROOM_SIZE - 1, ROOM_SIZE - 1),
        )
    }
}

/// A ball (square) with given center and radius (r=0 is a single tile, r=1 is 3x3).
pub fn ball(center: RoomXY, r: u8) -> Rect {
    unsafe {
        Rect {
            top_left: RoomXY::unchecked_new(
                if center.x.u8() <= r { 0 } else { center.x.u8() - r },
                if center.y.u8() <= r { 0 } else { center.y.u8() - r },
            ),
            bottom_right: RoomXY::unchecked_new(
                min(center.x.u8() + r, ROOM_SIZE - 1),
                min(center.y.u8() + r, ROOM_SIZE - 1),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::geometry::rect::Rect;
    use screeps::{RoomXY, ROOM_SIZE};
    #[test]
    fn test_iter() {
        let rect = unsafe {
            Rect::unchecked_new(
                RoomXY::unchecked_new(0, 0),
                RoomXY::unchecked_new(ROOM_SIZE - 1, 5),
            )
        };
        let mut number_of_points = 0;
        for xy in rect.iter() {
            number_of_points += 1
        }
        assert_eq!(number_of_points, rect.area());
        assert_eq!(rect.iter().next(), Some((0, 0).try_into().unwrap()));
    }
}
