use std::cmp::min;
use log::trace;
use screeps::{Position, RoomName};
use crate::hauling::issuing_requests::{with_haul_requests, RawStoreRequest, RawWithdrawRequest, RequestId};
use crate::u;

/// A structure containing matching requests to first withdraw and then store resources.
/// When dropped, then remaining requests are rescheduled.
pub struct MatchingRequests {
    pub withdraw_requests: Vec<(RequestId, RawWithdrawRequest)>,
    pub store_requests: Vec<(RequestId, RawStoreRequest)>,
}

impl Drop for MatchingRequests {
    fn drop(&mut self) {
        for (id, withdraw_request) in self.withdraw_requests.drain(..) {
            with_haul_requests(withdraw_request.room_name, |schedule| {
                schedule.withdraw_requests.insert(id, withdraw_request);
            });
        }

        for (id, store_request) in self.store_requests.drain(..) {
            with_haul_requests(store_request.room_name, |schedule| {
                schedule.store_requests.insert(id, store_request);
            });
        }
    }
}

/// Finds one or more withdraw requests and one or more store requests for given room (responsible
/// for providing the hauler) that are the current best option for a hauler with given position and
/// capacity to fulfull.
pub fn find_matching_requests(room_name: RoomName, creep_pos: Position, creep_capacity: u32) -> Option<MatchingRequests> {
    // TODO Do not pick up small amounts if it is under capacity and expected to increase later
    //      unless really needed.
    with_haul_requests(room_name, |schedule| {
        trace!("find_matching_requests({}, {}, {})", room_name, creep_pos, creep_capacity);
        trace!("withdraw_requests: {:?}", schedule.withdraw_requests);
        trace!("store_requests: {:?}", schedule.store_requests);

        if schedule.withdraw_requests.is_empty() || schedule.store_requests.is_empty() {
            return None;
        }

        let best_withdraw_request_data = schedule
            .withdraw_requests
            .iter()
            .filter_map(|(&id, request)| {
                // TODO Creep target.
                request.pos.map(|pos| {
                    let withdrawable_amount = min(creep_capacity, request.amount);
                    (id, pos, withdrawable_amount, pos.get_range_to(creep_pos))
                })
            })
            .min_by_key(|&(_, _, withdrawable_amount, d)| (u32::MAX - withdrawable_amount, d));

        if let Some((withdraw_request_id, withdraw_pos, _, _)) = best_withdraw_request_data {
            let best_store_request_data = schedule
                .store_requests
                .iter()
                .filter_map(|(&id, request)| request.pos.map(|xy| {
                    (id, xy.get_range_to(withdraw_pos))
                }))
                .min_by_key(|&(_, d)| d);

            if let Some((store_request_id, _)) = best_store_request_data {
                let withdraw_request = u!(schedule.withdraw_requests.remove(&withdraw_request_id));
                let store_request = u!(schedule.store_requests.remove(&store_request_id));
                
                return Some(MatchingRequests {
                    withdraw_requests: vec![(withdraw_request_id, withdraw_request)],
                    store_requests: vec![(store_request_id, store_request)],
                });
            }
        }
        
        None
    })
}