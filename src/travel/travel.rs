use crate::creeps::creeps::CreepRef;
use crate::kernel::broadcast::Broadcast;
use crate::local_debug;
use screeps::{FindPathOptions, Position};
use screeps::Path::Vectorized;
use screeps::pathfinder::MultiRoomCostResult;
use crate::creeps::creep::Creep;
use crate::errors::XiError;
use crate::creeps::creep_body::CreepBody;
use crate::geometry::position_utils::PositionUtils;
use crate::travel::step_utils::StepUtils;
use crate::travel::surface::Surface;
use crate::travel::travel_spec::TravelSpec;

const DEBUG: bool = true;

pub fn travel(creep_ref: &CreepRef, travel_spec: TravelSpec) -> Broadcast<Result<Position, XiError>> {
    let mut creep = creep_ref.borrow_mut();
    let creep_pos = creep.travel_state.pos;
    local_debug!("Creep {} travelling from {} to {}.", creep.name, creep_pos.f(), travel_spec.target.f());
    let options = FindPathOptions::<_, MultiRoomCostResult>::default()
        .ignore_creeps(true)
        .serialize(false);
    let path = creep_pos.find_path_to(&travel_spec.target, Some(options));
    local_debug!("Chosen path: {:?}.", path);
    // TODO Check if the path was actually found.
    creep.travel_state.spec = Some(travel_spec);
    if let Vectorized(mut path) = path {
        // The path coming from `find_path_to` is not a stack.
        path.reverse();
        let room_name = creep.travel_state.pos.room_name();
        creep.travel_state.path = path
            .into_iter()
            .map(|step| step.pos(room_name))
            .collect();
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
    surface: Surface
) -> u32 {
    let dist = (source.get_range_to(target) + 1).saturating_sub((start_range + range) as u32);
    let ticks_per_tile = body.ticks_per_tile(surface) as u32;
    dist * ticks_per_tile
}

/// Checks whether the creep is at the location specified by the travel spec.
/// If so, returns its position.
/// The creep must be alive. Its travel spec may not be `None`.
// TODO This function is weird. Remove it.
pub(crate) fn creep_arrival_pos(creep: &mut Creep) -> Option<Position> {
    let creep_pos = creep.travel_state.pos;
    creep.travel_state.spec.as_ref().and_then(|travel_spec| {
        travel_spec.is_in_target_rect(creep_pos).then_some(creep_pos)
    })
}
