use screeps::Position;
use crate::geometry::room_xy::RoomXYUtils;
#[cfg(test)]
use screeps::{RoomCoordinate, RoomName};

pub trait PositionUtils {
    fn same_room_around(self) -> impl Iterator<Item = Position>;
    
    fn f(&self) -> String;

    #[cfg(test)]
    fn new_from_raw(x: u8, y: u8, room_name: RoomName) -> Position;
}

impl PositionUtils for Position {
    #[inline]
    fn same_room_around(self) -> impl Iterator<Item = Position> {
        let room_name = self.room_name();
        self.xy().around().map(move |xy| xy.to_pos(room_name))
    }
    
    #[inline]
    fn f(&self) -> String {
        format!("({},{},{})", self.room_name(), self.x(), self.y())
    }
    
    #[cfg(test)]
    fn new_from_raw(x: u8, y: u8, room_name: RoomName) -> Position {
        unsafe {
            Position::new(
                RoomCoordinate::unchecked_new(x),
                RoomCoordinate::unchecked_new(y),
                room_name
            )
        }
    }
}