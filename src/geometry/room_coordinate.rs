use screeps::RoomCoordinate;

pub trait RoomCoordinateUtils {
    fn sub(self, other: Self) -> i8;
    unsafe fn add_diff(self, diff: i8) -> Self;
}

impl RoomCoordinateUtils for RoomCoordinate {
    fn sub(self, other: Self) -> i8 {
        (self.u8() as i8) - (other.u8() as i8)
    }

    unsafe fn add_diff(self, diff: i8) -> Self {
        RoomCoordinate::unchecked_new(((self.u8() as i8) + diff) as u8)
    }
}

#[cfg(test)]
mod tests {
    use screeps::RoomCoordinate;

    #[test]
    fn test_sub() {
        unsafe {
            assert_eq!(
                RoomCoordinate::unchecked_new(42).sub_diff(RoomCoordinate::unchecked_new(22)),
                20
            );
        }
    }
}
