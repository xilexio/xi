use crate::creeps::{for_each_creep, CreepRef};
use crate::kernel::condition::Broadcast;
use crate::kernel::sleep::sleep;
use crate::u;
use crate::utils::return_code_utils::ReturnCodeUtils;
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

pub fn travel(creep_ref: &CreepRef, travel_spec: TravelSpec) -> Broadcast<Result<Position, XiError>> {
    let mut creep = creep_ref.borrow_mut();
    creep.travel_state.spec = Some(travel_spec);
    if has_creep_arrived(&creep) {
        creep.travel_state.arrived = true;
        creep.travel_state.arrival_broadcast.broadcast(Ok(creep.pos()));
        creep.travel_state.arrival_broadcast.clone()
    } else {
        creep.travel_state.arrived = false;
        creep.travel_state.arrival_broadcast.reset();
        creep.travel_state.arrival_broadcast.clone()
    }
}

pub fn predicted_travel_ticks(source: Position, target: Position, start_range: u8, range: u8, body: &CreepBody) -> u32 {
    // TODO
    42
}

pub async fn move_creeps() {
    loop {
        for_each_creep(|creep_ref| {
            let mut creep = creep_ref.borrow_mut();
            if !creep.travel_state.arrived {
                if !creep.exists() {
                    creep.travel_state.arrival_broadcast.broadcast(Err(CreepDead));
                } else if has_creep_arrived(&creep) {
                    let creep_pos = creep.pos();
                    creep.travel_state.arrived = true;
                    creep.travel_state.arrival_broadcast.broadcast(Ok(creep_pos));
                } else {
                    let target = u!(creep.travel_state.spec.as_ref()).target;
                    creep
                        .move_to(target)
                        .to_bool_and_warn(&format!("Could not move creep {} towards {}", creep.name, target));
                }
            }
        });

        sleep(1).await;
    }
}

/// Checks whether the creep is at the location specified by the travel spec.
/// The travel spec may not be `None`.
fn has_creep_arrived(creep: &Creep) -> bool {
    let creep_pos = creep.pos();
    let travel_spec = u!(creep.travel_state.spec.as_ref());
    creep_pos.get_range_to(travel_spec.target) <= travel_spec.range as u32
}
