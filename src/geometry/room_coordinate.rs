use screeps::{OutOfBoundsError, RoomCoordinate};

pub trait RoomCoordinateUtils
where
    Self: Sized,
{
    fn sub(self, other: Self) -> i8;
    unsafe fn add_diff(self, diff: i8) -> Self;
    fn try_add_diff(self, diff: i8) -> Result<Self, OutOfBoundsError>;
}

impl RoomCoordinateUtils for RoomCoordinate {
    fn sub(self, other: Self) -> i8 {
        (self.u8() as i8) - (other.u8() as i8)
    }

    unsafe fn add_diff(self, diff: i8) -> Self {
        RoomCoordinate::unchecked_new(((self.u8() as i8) + diff) as u8)
    }

    fn try_add_diff(self, diff: i8) -> Result<Self, OutOfBoundsError> {
        RoomCoordinate::new((self.u8() as i8 + diff) as u8)
    }
}

#[cfg(test)]
mod tests {
    use screeps::RoomCoordinate;
    use crate::geometry::room_coordinate::RoomCoordinateUtils;

    #[test]
    fn test_sub() {
        unsafe {
            assert_eq!(
                RoomCoordinate::unchecked_new(42).sub(RoomCoordinate::unchecked_new(22)),
                20
            );
        }
    }
}
