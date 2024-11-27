use std::fmt::Display;
use screeps::Position;

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