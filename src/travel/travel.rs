use crate::creeps::{for_each_creep, CreepRef};
use crate::kernel::broadcast::Broadcast;
use crate::kernel::sleep::sleep;
use crate::{local_debug, u};
use crate::utils::result_utils::ResultUtils;
use screeps::{FindPathOptions, Position};
use screeps::Path::Vectorized;
use screeps::pathfinder::MultiRoomCostResult;
use crate::creeps::creep::Creep;
use crate::errors::XiError;
use crate::errors::XiError::CreepDead;
use crate::creeps::creep_body::CreepBody;
use crate::travel::travel_spec::TravelSpec;

const DEBUG: bool = true;

pub fn travel(creep_ref: &CreepRef, travel_spec: TravelSpec) -> Broadcast<Result<Position, XiError>> {
    let mut creep = creep_ref.borrow_mut();
    let creep_pos = u!(creep.pos());
    local_debug!("Creep {} travelling from {} to {}.", creep.name, creep_pos, travel_spec.target);
    let options = FindPathOptions::<_, MultiRoomCostResult>::default()
        .ignore_creeps(true)
        .serialize(false);
    let path = creep_pos.find_path_to(&travel_spec.target, Some(options));
    local_debug!("Chosen path: {:?}.", path);
    // TODO Check if the path was actually found.
    creep.travel_state.spec = Some(travel_spec);
    if let Vectorized(path) = path {
        creep.travel_state.path = path.into();
    } else {
        unreachable!();
    }
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
                    // TODO Can this happen?
                    local_debug!("Creep dead, not moving it.");
                    creep.travel_state.arrival_broadcast.broadcast(Err(CreepDead));
                } else if let Some(creep_pos) = creep_arrival_pos(&mut creep) {
                    local_debug!("Creep {} arrived at {}.", creep.name, creep_pos);
                    creep.travel_state.path.clear();
                    creep.travel_state.arrived = true;
                    creep.travel_state.arrival_broadcast.broadcast(Ok(creep_pos));
                } else {
                    let target = u!(creep.travel_state.spec.as_ref()).target;

                    local_debug!("Moving creep {} towards {}.", creep.name, target);

                    let path = &mut creep.travel_state.path;

                    if let Some(step) = path.pop_front() {
                        local_debug!("Next step to take: {:?}.", step);
                        creep
                            .move_direction(step.direction)
                            .warn_if_err(&format!("Could not move creep {} towards {}", creep.name, target));
                    }
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
