use screeps::{Position, RoomName, RoomXY, Step};
use crate::geometry::room_xy::RoomXYUtils;

pub trait StepUtils {
    fn xy(&self) -> RoomXY;
    
    fn pos(&self, room_name: RoomName) -> Position;
}

impl StepUtils for Step {
    #[inline]
    fn xy(&self) -> RoomXY {
        unsafe {
            RoomXY::unchecked_new(self.x as u8, self.y as u8)
        }
    }
    
    #[inline]
    fn pos(&self, room_name: RoomName) -> Position {
        self.xy().to_pos(room_name)
    }
}