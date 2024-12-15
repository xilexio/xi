use crate::creeps::creeps::CreepRef;
use crate::kernel::broadcast::Broadcast;
use crate::local_debug;
use screeps::{FindPathOptions, Position};
use screeps::Path::Vectorized;
use screeps::pathfinder::MultiRoomCostResult;
use crate::errors::XiError;
use crate::creeps::creep_body::CreepBody;
use crate::errors::XiError::PathNotFound;
use crate::geometry::position_utils::PositionUtils;
use crate::travel::step_utils::StepUtils;
use crate::travel::surface::Surface;
use crate::travel::travel_spec::TravelSpec;

const DEBUG: bool = true;

pub fn travel(creep_ref: &CreepRef, travel_spec: TravelSpec) -> Broadcast<Result<Position, XiError>> {
    let mut creep = creep_ref.borrow_mut();
    let creep_pos = creep.travel_state.pos;
    local_debug!(
        "Creep {} travelling from {} to {}.",
        creep.name, creep_pos.f(), travel_spec.target.f()
    );

    if travel_spec.is_in_target_rect(creep_pos) {
        creep.travel_state.spec = Some(travel_spec);
        creep.travel_state.arrived = true;
        creep.travel_state.arrival_broadcast.broadcast(Ok(creep_pos));
        creep.travel_state.arrival_broadcast.clone_primed()
    } else {
        creep.travel_state.arrived = false;
        
        match find_path(creep_pos, &travel_spec) {
            Ok(path) => {
                local_debug!("Chosen path: {:?}.", creep.travel_state.path);
                creep.travel_state.spec = Some(travel_spec);
                creep.travel_state.path = path;
                creep.travel_state.arrival_broadcast.reset();
            }
            Err(e) => {
                creep.travel_state.arrival_broadcast.broadcast(Err(e));
            }
        }
        
        creep.travel_state.arrival_broadcast.clone_primed()
    }
}

pub fn find_path(start_pos: Position, travel_spec: &TravelSpec) -> Result<Vec<Position>, XiError> {
    let options = FindPathOptions::<_, MultiRoomCostResult>::default()
        .ignore_creeps(true)
        .serialize(false);
    let steps = start_pos.find_path_to(&travel_spec.target, Some(options));
    local_debug!("Path from {} to {}: {:?}.", start_pos.f(), travel_spec.target.f(), steps);
    // TODO Check if the full path was actually found.
    if let Vectorized(mut steps) = steps {
        // TODO Multi-room travel.
        let room_name = start_pos.room_name();
        // Removing the last step while the second-to-last is in the target rect.
        while steps.get(steps.len() - 2).map_or(false, |step| travel_spec.is_in_target_rect(step.pos(room_name))) {
            steps.pop();
        }
        
        // The path coming from `find_path_to` is not a stack.
        steps.reverse();
        
        // TODO Multi-room travel.
        let path = steps.into_iter()
                .map(|step| step.pos(room_name))
                .collect::<Vec<_>>();
        
        if path.first().map_or(false, |&pos| travel_spec.is_in_target_rect(pos)) {
            Ok(path)
        } else {
            local_debug!("The last tile in the path is not in target rect. Only a partial path was found.");
            Err(PathNotFound)
        }
    } else {
        unreachable!();
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
    surface: Surface
) -> u32 {
    let dist = (source.get_range_to(target) + 1).saturating_sub((start_range + range) as u32);
    let ticks_per_tile = body.ticks_per_tile(surface) as u32;
    dist * ticks_per_tile
}