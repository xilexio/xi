use screeps::Direction;

#[inline]
pub fn offset_from_direction(direction: Direction) -> (i8, i8) {
    let i = direction as i8;
    (
        ((((i - 1) & 2) >> 1) | ((i - 1) & 1)) * (1 - (((i - 1) & 4) >> 1)),
        ((((i + 5) & 2) >> 1) | ((i + 5) & 1)) * (1 - (((i + 5) & 4) >> 1))
    )
}

#[cfg(test)]
mod tests {
    use screeps::Direction;
    use crate::geometry::direction::offset_from_direction;

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