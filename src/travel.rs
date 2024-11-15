use std::fmt::Display;
use log::trace;
use crate::creeps::{for_each_creep, CreepRef};
use crate::kernel::broadcast::Broadcast;
use crate::kernel::sleep::sleep;
use crate::u;
use crate::utils::result_utils::ResultUtils;
use screeps::Position;
use crate::creeps::creep::Creep;
use crate::errors::XiError;
use crate::errors::XiError::CreepDead;
use crate::creeps::creep::CreepBody;

#[derive(Debug)]
pub struct TravelState {
    /// Specification where the creep is supposed to be.
    spec: Option<TravelSpec>,
    /// Cached information whether the creep arrived at its destination and does not need to move.
    arrived: bool,
    /// Broadcast that the creep arrived at travel spec location.
    pub arrival_broadcast: Broadcast<Result<Position, XiError>>,
}

impl Default for TravelState {
    fn default() -> Self {
        TravelState {
            spec: None,
            arrived: true,
            arrival_broadcast: Broadcast::default(),
        }
    }
}

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

pub fn travel(creep_ref: &CreepRef, travel_spec: TravelSpec) -> Broadcast<Result<Position, XiError>> {
    let mut creep = creep_ref.borrow_mut();
    trace!("Creep {} travelling to {}", creep.name, travel_spec);
    creep.travel_state.spec = Some(travel_spec);
    if let Some(creep_pos) = creep_arrival_pos(&mut creep) {
        creep.travel_state.arrived = true;
        creep.travel_state.arrival_broadcast.broadcast(Ok(creep_pos));
        creep.travel_state.arrival_broadcast.clone_primed()
    } else {
        creep.travel_state.arrived = false;
        creep.travel_state.arrival_broadcast.reset();
        creep.travel_state.arrival_broadcast.clone_primed()
    }
}

/// Best effort estimate how many ticks it takes to travel `start_range` tiles from source to
/// `range` from target with a creep with given `body`. Takes into consideration if roads are
/// expected or not.
pub fn predicted_travel_ticks(
    source: Position,
    target: Position,
    start_range: u8,
    range: u8,
    body: &CreepBody,
    road: bool
) -> u32 {
    let dist = (source.get_range_to(target) + 1).saturating_sub((start_range + range) as u32);
    let ticks_per_tile = body.ticks_per_tile(road);
    dist * ticks_per_tile
}

pub async fn move_creeps() {
    loop {
        for_each_creep(|creep_ref| {
            let mut creep = creep_ref.borrow_mut();
            if !creep.travel_state.arrived {
                if creep.dead {
                    creep.travel_state.arrival_broadcast.broadcast(Err(CreepDead));
                } else if let Some(creep_pos) = creep_arrival_pos(&mut creep) {
                    creep.travel_state.arrived = true;
                    creep.travel_state.arrival_broadcast.broadcast(Ok(creep_pos));
                } else {
                    let target = u!(creep.travel_state.spec.as_ref()).target;
                    creep.move_to(target)
                        .warn_if_err(&format!("Could not move creep {} towards {}", creep.name, target));
                }
            }
        });

        sleep(1).await;
    }
}

/// Checks whether the creep is at the location specified by the travel spec.
/// If so, returns its position.
/// The creep must be alive. The travel spec may not be `None`.
fn creep_arrival_pos(creep: &mut Creep) -> Option<Position> {
    let creep_pos = u!(creep.pos());
    let travel_spec = u!(creep.travel_state.spec.as_ref());
    if creep_pos.get_range_to(travel_spec.target) <= travel_spec.range as u32 {
        Some(creep_pos)
    } else {
        None
    }
}
