use std::error::Error;
use std::fmt::{Display, Formatter};
use screeps::RoomXY;

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

    pub fn inside(self, xy: RoomXY) -> bool {
        self.top_left.x <= xy.x
            && xy.x <= self.bottom_right.x
            && self.top_left.y <= xy.y
            && xy.y <= self.bottom_right.y
    }
}

impl TryFrom<(RoomXY, RoomXY)> for Rect {
    type Error = InvalidRectError;

    fn try_from(xy_pair: (RoomXY, RoomXY)) -> Result<Self, Self::Error> {
        Rect::new(xy_pair.0, xy_pair.1)
    }
}