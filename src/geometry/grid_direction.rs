use enum_iterator::Sequence;
use crate::geometry::grid_direction::GridDirection::*;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Sequence)]
#[repr(u8)]
pub enum GridDirection {
    Center = 0,
    Top = 1,
    TopRight = 2,
    Right = 3,
    BottomRight = 4,
    Bottom = 5,
    BottomLeft = 6,
    Left = 7,
    TopLeft = 8,
}

impl From<u8> for GridDirection {
    fn from(value: u8) -> Self {
        match value {
            0 => Center,
            1 => Top,
            2 => TopRight,
            3 => Right,
            4 => BottomRight,
            5 => Bottom,
            6 => BottomLeft,
            7 => Left,
            8 => TopLeft,
            _ => unreachable!(),
        }
    }
}

#[inline]
pub fn reverse_direction(direction: GridDirection) -> GridDirection {
    if direction == Center {
        direction
    } else {
        ((direction as u8 - 1 + 4) % 8 + 1).into()
    }
}

#[inline]
pub fn direction_to_offset(direction: GridDirection) -> (i8, i8) {
    match direction {
        Center => (0, 0),
        Top => (0, -1),
        TopRight => (1, -1),
        Right => (1, 0),
        BottomRight => (1, 1),
        Bottom => (0, 1),
        BottomLeft => (-1, 1),
        Left => (-1, 0),
        TopLeft => (-1, -1),
    }
}