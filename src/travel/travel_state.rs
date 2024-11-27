use std::collections::VecDeque;
use screeps::{Position, Step};
use crate::errors::XiError;
use crate::kernel::broadcast::Broadcast;
use crate::travel::travel_spec::TravelSpec;

#[derive(Debug)]
pub struct TravelState {
    /// Specification where the creep is supposed to be.
    pub(crate) spec: Option<TravelSpec>,
    pub(crate) path: VecDeque<Step>,
    /// Cached information whether the creep arrived at its destination and does not need to move.
    pub(crate) arrived: bool,
    /// Broadcast that the creep arrived at travel spec location.
    pub arrival_broadcast: Broadcast<Result<Position, XiError>>,
}

impl Default for TravelState {
    fn default() -> Self {
        TravelState {
            spec: None,
            path: VecDeque::default(),
            arrived: true,
            arrival_broadcast: Broadcast::default(),
        }
    }
}