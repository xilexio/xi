use std::fmt::Display;
use screeps::Position;
use crate::geometry::rect::{ball, Rect};

// TODO Information whether the creep can be shoved off the target rect after reaching it.
#[derive(Debug, Clone)]
pub struct TravelSpec {
    pub target: Position,
    pub range: u8,
}

impl Display for TravelSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{} (range: {})", self.target.room_name(), self.target.xy(), self.range)
    }
}

impl TravelSpec {
    pub fn target_rect(&self) -> Rect {
        ball(self.target.xy(), self.range)
    }
    
    pub fn is_in_target_rect(&self, pos: Position) -> bool {
        pos.get_range_to(self.target) <= self.range as u32
    }
}