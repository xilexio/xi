use std::cmp::min;
use log::trace;
use screeps::{Position, RoomName};
use crate::hauling::issuing_requests::{with_haul_requests, RawStoreRequest, RawWithdrawRequest, RequestId};
use crate::hauling::issuing_requests::RequestAmountChange::Increase;
use crate::{local_trace, u};

const DEBUG: bool = true;

/// Not taking into consideration picking up decaying resources under this amount.
const MIN_DECAYING_AMOUNT: u32 = 100;

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
                local_trace!("Rescheduling dropped withdraw request {}.", withdraw_request);
                schedule.withdraw_requests.insert(id, withdraw_request);
            });
        }

        for (id, store_request) in self.store_requests.drain(..) {
            with_haul_requests(store_request.room_name, |schedule| {
                local_trace!("Rescheduling dropped store request {}.", store_request);
                schedule.store_requests.insert(id, store_request);
            });
        }
    }
}

/// Finds one or more withdraw requests and one or more store requests for given room (responsible
/// for providing the hauler) that are the current best option for a hauler with given position and
/// capacity to fulfill.
pub fn find_matching_requests(
    room_name: RoomName,
    creep_pos: Position,
    creep_capacity: u32,
    carried_energy: u32
) -> Option<MatchingRequests> {
    // TODO Do not pick up small amounts if it is under capacity and expected to increase later
    //      unless really needed.
    with_haul_requests(room_name, |schedule| {
        if DEBUG {
            trace!("Finding matching requests in {} for pos {} and capacity {}.", room_name, creep_pos, creep_capacity);
            trace!("Available withdraw requests:");
            for request in schedule.withdraw_requests.values() {
                trace!("* {}", request);
            }
            trace!("Available store requests:");
            for request in schedule.store_requests.values() {
                trace!("* {}", request);
            }
        }

        if schedule.withdraw_requests.is_empty() || schedule.store_requests.is_empty() {
            return None;
        }

        let best_withdraw_request_data = schedule
            .withdraw_requests
            .iter()
            .filter_map(|(&id, request)| {
                // TODO Creep target.
                // TODO Prioritize decaying resources at the expense of distance, but only if no one
                //      else who wants it is closer.
                request.pos.and_then(|pos| {
                    if request.amount_change == Increase && request.amount < creep_capacity {
                        // Not taking something which amount will only increase and is under creep
                        // carry capacity.
                        return None;
                    }

                    let dist = pos.get_range_to(creep_pos);
                    if request.amount_change != Increase && request.amount - request.decay * dist < MIN_DECAYING_AMOUNT {
                        return None;
                    }

                    let withdrawable_amount = min(creep_capacity, request.amount);
                    Some((id, pos, withdrawable_amount, dist, request.decay))
                })
            })
            .min_by_key(|&(_, _, withdrawable_amount, dist, decay)| (-(withdrawable_amount as i32), dist, -(decay as i32)));

        if let Some((withdraw_request_id, withdraw_pos, _, _, _)) = best_withdraw_request_data {
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