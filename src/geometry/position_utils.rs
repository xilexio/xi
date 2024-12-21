use screeps::{RoomCoordinate, Position, ROOM_SIZE};
use crate::geometry::room_xy::RoomXYUtils;
use crate::u;
#[cfg(test)]
use screeps::RoomName;

pub trait PositionUtils {
    fn same_room_around(self) -> impl Iterator<Item = Self>;
    
    /// A connected tile from the neighboring room to which creeps entering this tile are
    /// teleported. Undefined behavior for corners.
    fn matching_boundary_pos(self) -> Self;
    
    fn f(&self) -> String;

    #[cfg(test)]
    fn new_from_raw(x: u8, y: u8, room_name: RoomName) -> Self;
}

impl PositionUtils for Position {
    #[inline]
    fn same_room_around(self) -> impl Iterator<Item = Self> {
        let room_name = self.room_name();
        self.xy().around().map(move |xy| xy.to_pos(room_name))
    }

    fn matching_boundary_pos(self) -> Self {
        if self.x().u8() == 0 {
            unsafe {
                Self::new(
                    RoomCoordinate::unchecked_new(ROOM_SIZE - 1),
                    self.y(),
                    u!(self.room_name().checked_add((-1, 0)))
                )
            }
        } else if self.x().u8() == ROOM_SIZE - 1 {
            unsafe {
                Self::new(
                    RoomCoordinate::unchecked_new(0),
                    self.y(),
                    u!(self.room_name().checked_add((1, 0)))
                )
            }
        } else if self.y().u8() == 0 {
            unsafe {
                Self::new(
                    self.x(),
                    RoomCoordinate::unchecked_new(ROOM_SIZE - 1),
                    u!(self.room_name().checked_add((0, -1)))
                )
            }
        } else if self.y().u8() == ROOM_SIZE - 1 {
            unsafe {
                Self::new(
                    self.x(),
                    RoomCoordinate::unchecked_new(0),
                    u!(self.room_name().checked_add((0, 1)))
                )
            }
        } else {
            unreachable!()
        }
    }

    #[inline]
    fn f(&self) -> String {
        format!("({},{},{})", self.room_name(), self.x(), self.y())
    }
    
    #[cfg(test)]
    fn new_from_raw(x: u8, y: u8, room_name: RoomName) -> Self {
        unsafe {
            Self::new(
                RoomCoordinate::unchecked_new(x),
                RoomCoordinate::unchecked_new(y),
                room_name
            )
        }
    }
}