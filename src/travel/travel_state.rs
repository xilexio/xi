use screeps::Position;
use crate::errors::XiError;
use crate::kernel::broadcast::Broadcast;
use crate::travel::travel_spec::TravelSpec;

#[derive(Debug)]
pub struct TravelState {
    /// Current position, updated near the beginning of the tick.
    pub pos: Position,
    /// Specification where the creep is supposed to be.
    pub(crate) spec: Option<TravelSpec>,
    /// Path in the form of stack.
    pub(crate) path: Vec<Position>,
    /// Cached information whether the creep arrived at its destination and does not need to move.
    pub(crate) arrived: bool,
    /// Broadcast that the creep arrived at travel spec location.
    pub arrival_broadcast: Broadcast<Result<Position, XiError>>,
}

impl TravelState {
    pub fn new(pos: Position) -> Self {
        TravelState {
            pos,
            spec: None,
            path: Vec::default(),
            arrived: true,
            arrival_broadcast: Broadcast::default(),
        }
    }
    
    pub fn next_pos(&mut self) -> Position {
        self.path.last().cloned().unwrap_or(self.pos)
    }
    
    pub fn is_in_target_rect(&self) -> bool {
        if let Some(spec) = &self.spec {
            spec.is_in_target_rect(self.pos)
        } else {
            true
        }
    }
}