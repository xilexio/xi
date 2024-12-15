use std::cmp::{min, Reverse};
use log::debug;
use rustc_hash::FxHashMap;
use screeps::{Position, ResourceType, RoomName};
use crate::{local_debug, u};
use crate::geometry::position_utils::PositionUtils;
use crate::hauling::requests::{with_haul_requests, ReservedHaulRequest};
use crate::hauling::requests::HaulRequestTargetKind::StorageTarget;
use crate::hauling::requests::RequestAmountChange::Increase;

const DEBUG: bool = true;

/// Not taking into consideration picking up decaying resources under this amount.
const MIN_DECAYING_AMOUNT: u32 = 100;

const CREEP_LOW_TTL: u32 = 100;

/// A structure containing active requests to first withdraw and then store resources.
/// When dropped, the remaining requests are rescheduled.
/// The contents of the requests may change on the way. Specifically, the amount and position
/// (when the target is a creep) is expected to change. Resource type, target and whether it is
/// a pickup may not change.
/// When it changes, the hauler's plans may need to be updated.
/// Note that due to requests from piles being replaced, one cannot rely on ID being the same.
pub struct ReservedRequests {
    pub withdraw_requests: Vec<ReservedHaulRequest>,
    pub deposit_requests: Vec<ReservedHaulRequest>,
}

/// Finds one or more withdraw and/or deposit requests for given room (responsible for providing
/// the hauler) that are the current best option to fulfill for a hauler with given store and
/// position.
/* Plan for the algorithm:
There are withdraw (includes pickup) and store (transfer from creep) requests. They have information
about whether the amount is supposed to increase, decrease, stay the same or be erratic. Also, they
have information whether these requests are to a permanent storage to prevent moving resources
back and forth between storages or the same storage.

We start with a creep in the idle state. The creep finds a withdraw or store request it can fulfill
with the least total distance. If there are no such requests, it depends.
If the creep is close to death, it tries to fulfill a storage store request and suicide.
If the creep has non-energy or there are any withdraw requests that it ignored just because they
are too small (it indicates that fulfillable request work appear later), it may fulfill storage
store requests. In particular, a creep with energy and sufficient TTL can normally wait idly in a
room with link mining instead of using intents to move to storage, all while possibly being near
something that will need filling soon.

Withdraw requests marked as storage are ignored as they are meant to be paired with store requests.
There are two kinds of other withdraw requests.

One is a non-storage withdraw with increasing amount, e.g., from mining a source or mineral. In this
case, the hauler is supposed to withdraw the resource only when there is enough to fully fill it.

The other case is a withdraw or pickup of some loose thing or a product, e.g., a factory product,
resource pile or tomb. In this case, the hauler is supposed to simply withdraw it regardless of the
amount, though it may be ignored forever if the amount is small compared to the distance. If it is
to be picked up and is decaying, it has higher priority (as constant distance decrease) to account
for resources lost delaying the pickup and the amount lost when travelling is taken into account
when deciding whether to pick it up.
The distance to a withdraw request is just the distance to the target. After withdrawing the
resources, the creep enters idle state.

Store requests marked as storage are ignored as they are meant to be paired with withdraw requests.
Normal store requests require pairing them with withdraw requests unless the creep already has
resources on hand. If it has them all, it just performs the request. Otherwise, it depends.
If the creep has no resources, it first finds a withdraw request, including one from a storage.
If the creep has insufficient resources, and the store request is within carry capacity, it finds
a withdraw request to top up first, including storage one.
If the creep has insufficient resources and the store request exceeds carry capacity, but adding
whatever it has on hand would make the required amount under carry capacity, it partially fulfills
the request.
If that would still be insufficient, it either first finds a storage withdraw request or partially
fulfills the request depending on which of these (computed approximately) is higher:
(amount on hand) / (distance to target + distance from target to storage)
or
(carry capacity) / (distance to storage + 2 * distance from storage to target).
The total distance is the distance to the withdraw request plus the distance to the target or just
the distance to the target if no withdraw request was used.
 */
// TODO Pick up smaller piles when there is nothing else to do. Or just move towards expected energy
//      source or storage.
pub fn find_haul_requests(
    room_name: RoomName,
    creep_store: &FxHashMap<ResourceType, u32>,
    creep_pos: Position,
    creep_capacity: u32,
    creep_ttl: u32
) -> Option<ReservedRequests> {
    with_haul_requests(room_name, |haul_requests| {
        if DEBUG {
            let resources_str = if creep_store.is_empty() {
                "no resources".into()
            } else {
                creep_store
                    .iter()
                    .map(|(resource, &amount)| format!("{:?}: {}", resource, amount))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            debug!(
                "Finding haul requests in {} for creep with {}, capacity {} and {} TTL at {}.",
                room_name,
                resources_str,
                creep_capacity,
                creep_pos.f(),
                creep_ttl
            );
        }

        let mut withdraw_requests = Vec::new();
        let mut deposit_requests = Vec::new();

        if let Some(&first_resource_type) = creep_store.keys().next() {
            // Filled creep store. In this case, the creep seeks to deposit its resources somewhere.

            // First trying to find a non-storage deposit request that can be fulfilled with
            // what is already carried. For example, to deposit the picked up energy from
            // drop mining.
            // If this fails, trying to find a deposit request to storage, but only if the creep
            // has a non-energy resource or is low on TTL.
            let storage_possible = creep_ttl < CREEP_LOW_TTL || first_resource_type != ResourceType::Energy || creep_store.len() >= 2;

            let deposit_request_data = haul_requests
                .deposit_requests
                .iter()
                .filter_map(|(&id, request)| {
                    let borrowed_request = request.borrow();
                    let is_storage = borrowed_request.target_kind == StorageTarget;
                    if !storage_possible && is_storage {
                        return None;
                    }
                    let carried_amount = if let Some(&amount) = creep_store.get(&borrowed_request.resource_type) {
                        amount
                    } else {
                        return None;
                    };
                    let depositable_amount = min(carried_amount as i32, borrowed_request.unreserved_amount());
                    if depositable_amount <= 0 {
                        return None;
                    }
                    // TODO Reward requests with higher amount.
                    // TODO Penalize requests that would not be completely fulfilled unless
                    //      the request itself is already over capacity.
                    // TODO Penalize requests such that fulfilling possible amount would not change
                    //      the number of full capacities to withdraw them.
                    // TODO Also include all possible requests available when standing on one of
                    //      neighboring tiles (e.g., a group of up to 6 more extensions).
                    Some((id, depositable_amount as u32, is_storage, borrowed_request.pos.get_range_to(creep_pos)))
                })
                .max_by_key(|&(_, depositable_amount, is_storage, dist)| (is_storage, Reverse(dist), depositable_amount));

            if let Some((request_id, depositable_amount, _, _)) = deposit_request_data {
                local_debug!("Found deposit request {:?} for {}.", request_id, depositable_amount);
                deposit_requests.push((request_id, depositable_amount));
            }
        } else {
            // Empty creep store. In this case, the creep seeks to withdraw resources from somewhere.
            // It will not withdraw from a storage unless there is a non-storage deposit request.
            // but only if either there is somewhere to put them or they must be withdrawn to not be
            // lost (i.e., are not in storage).

            // First trying to find a non-storage withdraw request that fills up the creep or is not
            // increasing in amount.
            let withdraw_request_data = haul_requests
                .withdraw_requests
                .iter()
                .filter_map(|(&id, request)| {
                    let borrowed_request = request.borrow();
                    if borrowed_request.target_kind == StorageTarget {
                        return None;
                    }
                    let withdrawable_amount = min(creep_capacity as i32, borrowed_request.unreserved_amount());
                    if withdrawable_amount <= 0 {
                        return None;
                    }
                    // Not undertaking increasing requests that do not fill the creep.
                    if borrowed_request.amount_change == Increase && (withdrawable_amount as u32) < creep_capacity {
                        return None;
                    }
                    // TODO Reward requests with higher amount.
                    // TODO Ignore too small requests from loose piles and let them decay.
                    // TODO Reward decaying requests if deciding to pick them up.
                    // TODO Also include all possible requests available when standing on one of
                    //      neighboring tiles.
                    Some((id, withdrawable_amount as u32, borrowed_request.pos.get_range_to(creep_pos)))
                })
                .max_by_key(|&(_, withdrawable_amount, dist)| (Reverse(dist), withdrawable_amount));

            if let Some((request_id, withdrawable_amount, _)) = withdraw_request_data {
                local_debug!("Found withdraw request {:?} for {}.", request_id, withdrawable_amount);
                withdraw_requests.push((request_id, withdrawable_amount));
            } else {
                // If there is no non-storage withdraw request, try to find a deposit request and
                // a withdraw request from storage.

                let eligible_storage_withdraw_request_data = haul_requests
                    .withdraw_requests
                    .iter()
                    .filter_map(|(&id, request)| {
                        let borrowed_request = request.borrow();
                        // Non-storage requests were already processed.
                        if borrowed_request.target_kind != StorageTarget {
                            return None;
                        }
                        let withdrawable_amount = min(creep_capacity as i32, borrowed_request.unreserved_amount());
                        if withdrawable_amount <= 0 {
                            return None;
                        }
                        Some((id, withdrawable_amount as u32, borrowed_request.resource_type, borrowed_request.pos, borrowed_request.pos.get_range_to(creep_pos)))
                    })
                    .collect::<Vec<_>>();

                let withdraw_and_deposit_request_data = haul_requests
                    .deposit_requests
                    .iter()
                    .filter_map(|(&deposit_request_id, request)| {
                        let borrowed_request = request.borrow();
                        if borrowed_request.target_kind == StorageTarget {
                            return None;
                        }
                        let max_depositable_amount = min(creep_capacity as i32, borrowed_request.unreserved_amount());
                        if max_depositable_amount <= 0 {
                            return None;
                        }
                        let withdraw_request_data = eligible_storage_withdraw_request_data
                            .iter()
                            .filter_map(|&(withdraw_request_id, withdrawable_amount, resource_type, withdraw_pos, withdraw_dist)| {
                                if borrowed_request.resource_type != resource_type {
                                    return None;
                                }

                                let amount = min(max_depositable_amount as u32, withdrawable_amount);
                                let total_dist = withdraw_dist + withdraw_pos.get_range_to(borrowed_request.pos);

                                Some((withdraw_request_id, amount, total_dist))
                            })
                            .max_by_key(|&(_, amount, total_dist)| (Reverse(total_dist), amount));

                        withdraw_request_data.map(|(withdraw_request_id, amount, total_dist)| {
                            // When it is energy, withdrawing the full capacity. When something
                            // else, withdrawing exactly as much as is needed.
                            let withdrawable_amount = if borrowed_request.resource_type == ResourceType::Energy {
                                creep_capacity
                            } else {
                                amount
                            };
                            (withdraw_request_id, deposit_request_id, withdrawable_amount, amount, total_dist)
                        })
                    })
                    .max_by_key(|&(_, _, _, depositable_amount, total_dist)| (Reverse(total_dist), depositable_amount));

                if let Some((withdraw_request_id, deposit_request_id, withdrawable_amount, depositable_amount, _)) = withdraw_and_deposit_request_data {
                    // TODO Maybe not always take full creep capacity of minerals?
                    local_debug!(
                        "Found withdraw request {:?} for {} and deposit request {:?} for {}.",
                        withdraw_request_id,
                        withdrawable_amount,
                        deposit_request_id,
                        depositable_amount
                    );
                    withdraw_requests.push((withdraw_request_id, withdrawable_amount));
                    deposit_requests.push((deposit_request_id, depositable_amount));
                }
            }
        }

        (!withdraw_requests.is_empty() || !deposit_requests.is_empty()).then(|| {
            let reserved_withdraw_requests = withdraw_requests
                .into_iter()
                .map(|(withdraw_request_id, amount)| {
                    ReservedHaulRequest::new(
                        u!(haul_requests.withdraw_requests.get(&withdraw_request_id)).clone(),
                        amount
                    )
                })
                .collect();

            let reserved_deposit_requests = deposit_requests
                .into_iter()
                .map(|(deposit_request_id, amount)| {
                    ReservedHaulRequest::new(
                        u!(haul_requests.deposit_requests.get(&deposit_request_id)).clone(),
                        amount
                    )
                })
                .collect();

            Some(ReservedRequests {
                withdraw_requests: reserved_withdraw_requests,
                deposit_requests: reserved_deposit_requests,
            })
        })
    }).flatten()
}