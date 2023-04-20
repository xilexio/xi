use screeps::Direction;

pub const OFFSET_BY_DIRECTION: [(i8, i8); 9] = [
    (0, 0),
    (0, -1),
    (1, -1),
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
    (-1, -1),
];

pub fn rotate_clockwise(direction: Direction) -> Direction {
    match direction {
        Direction::Top => Direction::TopRight,
        Direction::TopRight => Direction::Right,
        Direction::Right => Direction::BottomRight,
        Direction::BottomRight => Direction::Bottom,
        Direction::Bottom => Direction::BottomLeft,
        Direction::BottomLeft => Direction::Left,
        Direction::Left => Direction::TopLeft,
        Direction::TopLeft => Direction::Top,
    }
}

pub fn rotate_counterclockwise(direction: Direction) -> Direction {
    match direction {
        Direction::Top => Direction::TopLeft,
        Direction::TopRight => Direction::Top,
        Direction::Right => Direction::TopRight,
        Direction::BottomRight => Direction::Right,
        Direction::Bottom => Direction::BottomRight,
        Direction::BottomLeft => Direction::Bottom,
        Direction::Left => Direction::BottomLeft,
        Direction::TopLeft => Direction::Left,
    }
}

pub fn add_offsets(offset1: (i8, i8), offset2: (i8, i8)) -> (i8, i8) {
    (offset1.0 + offset2.0, offset1.1 + offset2.1)
}

pub fn mul_offsets(offset: (i8, i8), multiplier: i8) -> (i8, i8) {
    (offset.0 * multiplier, offset.1 * multiplier)
}

#[inline]
pub fn offset_from_direction(direction: Direction) -> (i8, i8) {
    let i = direction as i8;
    (
        ((((i - 1) & 2) >> 1) | ((i - 1) & 1)) * (1 - (((i - 1) & 4) >> 1)),
        ((((i + 5) & 2) >> 1) | ((i + 5) & 1)) * (1 - (((i + 5) & 4) >> 1)),
    )
}

#[cfg(test)]
mod tests {
    use crate::geometry::direction::offset_from_direction;
    use screeps::Direction;

    #[test]
    fn test_offset_from_direction() {
        assert_eq!(offset_from_direction(Direction::Top), (0, -1));
        assert_eq!(offset_from_direction(Direction::TopRight), (1, -1));
        assert_eq!(offset_from_direction(Direction::Right), (1, 0));
        assert_eq!(offset_from_direction(Direction::BottomRight), (1, 1));
        assert_eq!(offset_from_direction(Direction::Bottom), (0, 1));
        assert_eq!(offset_from_direction(Direction::BottomLeft), (-1, 1));
        assert_eq!(offset_from_direction(Direction::Left), (-1, 0));
        assert_eq!(offset_from_direction(Direction::TopLeft), (-1, -1));
    }
}
