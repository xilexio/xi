use std::fmt::Display;
use screeps::Position;
use crate::geometry::rect::{ball, Rect};
use crate::utils::priority::Priority;

// TODO Information whether the creep can be shoved off the target rect after reaching it.
#[derive(Debug, Clone)]
pub struct TravelSpec {
    pub target: Position,
    pub range: u8,
    /// The primary cost of movement is the progress cost. In general, a conflicting creep will
    /// only move into its desired tile if its progress priority is higher than the sum of
    /// other lost progress priorities. 
    pub progress_priority: Priority,
    /// The priority cost of being moved out of the target rect after already being inside it.
    pub target_rect_priority: Priority,
}

impl Display for TravelSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{} (range: {})", self.target.room_name(), self.target.xy(), self.range)
    }
}

impl TravelSpec {
    pub fn new(target: Position, range: u8) -> Self {
        Self {
            target,
            range,
            progress_priority: Priority(80),
            target_rect_priority: Priority(160),
        }
    }
    
    pub fn target_rect(&self) -> Rect {
        ball(self.target.xy(), self.range)
    }
    
    pub fn is_in_target_rect(&self, pos: Position) -> bool {
        pos.get_range_to(self.target) <= self.range as u32
    }
    
    pub fn with_progress_priority(mut self, priority: Priority) -> Self {
        self.progress_priority = priority;
        self
    }
    
    pub fn with_target_rect_priority(mut self, priority: Priority) -> Self {
        self.target_rect_priority = priority;
        self
    }
}