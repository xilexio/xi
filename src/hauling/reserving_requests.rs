use std::cmp::{min, Reverse};
use log::debug;
use screeps::{Position, ResourceType, RoomName};
use crate::{a, u};
use crate::hauling::requests::{with_haul_requests, ReservedHaulRequest};
use crate::hauling::requests::RequestAmountChange::Increase;

const DEBUG: bool = true;

/// Not taking into consideration picking up decaying resources under this amount.
const MIN_DECAYING_AMOUNT: u32 = 100;

/// A structure containing active requests to first withdraw and then store resources.
/// When dropped, the remaining requests are rescheduled.
/// The contents of the requests may change on the way. Specifically, the amount and position
/// (when the target is a creep) is expected to change. Resource type, target and whether it is
/// a pickup may not change.
/// When it changes, the hauler's plans may need to be updated.
/// Note that due to requests from piles being replaced, one cannot rely on ID being the same.
pub struct ReservedRequests {
    pub withdraw_requests: Vec<ReservedHaulRequest>,
    pub store_requests: Vec<ReservedHaulRequest>,
}

/// Finds one or more withdraw requests and one or more store requests for given room (responsible
/// for providing the hauler) that are the current best option for a hauler with given position and
/// capacity to fulfill.
pub fn find_matching_requests(
    room_name: RoomName,
    creep_pos: Position,
    creep_capacity: u32,
    carried_energy: u32
) -> Option<ReservedRequests> {
    // TODO Do not pick up small amounts if it is under capacity and expected to increase later
    //      unless really needed.
    with_haul_requests(room_name, |haul_requests| {
        if DEBUG {
            debug!("Finding matching requests in {} for pos {} and capacity {} and {} carried energy.",
                room_name, creep_pos, creep_capacity, carried_energy);
            debug!("Available withdraw requests:");
            for request in haul_requests.withdraw_requests.values() {
                debug!("* {}", request.borrow());
            }
            debug!("Available store requests:");
            for request in haul_requests.store_requests.values() {
                debug!("* {}", request.borrow());
            }
        }

            
        let best_withdraw_request_data = (creep_capacity > carried_energy)
            .then_some(haul_requests
                .withdraw_requests
                .iter()
                .filter_map(|(&id, request)| {
                    let borrowed_request = request.borrow();
                    let unreserved_amount = borrowed_request.unreserved_amount();
                    if unreserved_amount <= 0 {
                        return None;
                    }
    
                    if borrowed_request.resource_type != ResourceType::Energy {
                        // TODO Support for other resource types.
                        return None;
                    }
    
                    // TODO Creep target.
                    // TODO Prioritize decaying resources at the expense of distance, but only if no one
                    //      else who wants it is closer.
                    if borrowed_request.amount_change == Increase && (unreserved_amount as u32) < creep_capacity {
                        // Not taking something which amount will only increase and is under creep
                        // carry capacity.
                        return None;
                    }
    
                    let dist = borrowed_request.pos.get_range_to(creep_pos);
                    if borrowed_request.amount_change != Increase && borrowed_request.amount - borrowed_request.decay * dist < min(creep_capacity, MIN_DECAYING_AMOUNT) {
                        return None;
                    }
    
                    let withdrawable_amount = min(creep_capacity - carried_energy, unreserved_amount as u32);
                    Some((id, borrowed_request.pos, withdrawable_amount, dist, borrowed_request.decay))
                })
                .max_by_key(|&(_, _, withdrawable_amount, dist, decay)| (withdrawable_amount, Reverse(dist), decay))
        ).flatten();

        if let Some((withdraw_request_id, withdraw_pos, withdrawable_amount, _, _)) = best_withdraw_request_data {
            let total_available_amount = carried_energy + withdrawable_amount;
            a!(total_available_amount <= creep_capacity);

            let best_store_request_data = haul_requests
                .store_requests
                .iter()
                .filter_map(|(&id, request)| {
                    let borrowed_request = request.borrow();
                    let storable_amount = min(total_available_amount as i32, borrowed_request.unreserved_amount());
                    (storable_amount > 0).then_some(
                        (id, storable_amount as u32, borrowed_request.pos.get_range_to(withdraw_pos))
                    )
                })
                .max_by_key(|&(_, storable_amount, dist)| (storable_amount, Reverse(dist)));

            if let Some((store_request_id, storable_amount, _)) = best_store_request_data {
                let mut withdraw_requests = Vec::new();
                if carried_energy < storable_amount {
                    withdraw_requests.push(ReservedHaulRequest::new(
                        u!(haul_requests.withdraw_requests.get(&withdraw_request_id)).clone(),
                        storable_amount - carried_energy
                    ));
                }
                
                let mut store_requests = Vec::new();
                store_requests.push(ReservedHaulRequest::new(
                    u!(haul_requests.store_requests.get(&store_request_id)).clone(),
                    storable_amount,
                ));

                return Some(ReservedRequests {
                    withdraw_requests,
                    store_requests,
                });
            }
        }

        None
    })
}