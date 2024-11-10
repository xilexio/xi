use crate::creeps::creep::{CreepBody, CreepRole};
use crate::creeps::CreepRef;
use crate::errors::XiError;
use crate::game_tick::game_tick;
use crate::geometry::room_xy::RoomXYUtils;
use crate::hauling::requests::{with_haul_requests, RawStoreRequest, RawWithdrawRequest};
use crate::kernel::sleep::sleep;
use crate::priorities::HAULER_SPAWN_PRIORITY;
use crate::room_state::room_states::with_room_state;
use crate::spawn_pool::{SpawnPool, SpawnPoolOptions};
use crate::spawning::{PreferredSpawn, SpawnRequest};
use crate::travel::{travel, TravelSpec};
use crate::u;
use log::debug;
use screeps::Part::{Carry, Move};
use screeps::StructureType::Storage;
use screeps::{ResourceType, RoomName};
use std::iter::repeat;
use std::vec;
use crate::creeps::actions::{pickup_when_able, transfer_when_able, withdraw_when_able};
use crate::hauling::store_anywhere_or_drop::store_anywhere_or_drop;
use crate::room_state::RoomState;

/// Execute hauling of resources of haulers assigned to given room.
/// Withdraw and store requests are registered in the system and the system assigns them to fre
/// haulers. One or more withdraw event is paired with one or more store events. There are special
/// withdraw and store events for the storage which may not be paired with one another.
pub async fn haul_resources(room_name: RoomName) {
    let base_spawn_request = u!(with_room_state(room_name, |room_state| {
        let body = hauler_body(room_state);

        // Any spawn is good.
        // TODO Remove directions reserved for the fast filler.
        let preferred_spawns = room_state
            .spawns
            .iter()
            .map(|spawn_data| PreferredSpawn {
                id: spawn_data.id,
                directions: Vec::new(),
                extra_cost: 0,
            })
            .collect::<Vec<_>>();

        SpawnRequest {
            role: CreepRole::Hauler,
            body,
            priority: HAULER_SPAWN_PRIORITY,
            preferred_spawns,
            tick: (0, 0),
        }
    }));

    let mut spawn_pools = Vec::new();
    
    loop {
        let required_haulers = u!(with_room_state(room_name, |room_state| {
            room_state
            .eco_config
            .as_ref()
            .map(|distribution| distribution.haulers_required)
            .unwrap_or(1)
        }));

        // TODO Not spawning the replacement creep when the number of required haulers is lower than
        //      the number of existing spawn pools. Instead of maintaining a few spawn pools, this
        //      should be a single spawn pool handling multiple creeps.
        if spawn_pools.len() < required_haulers as usize {
            spawn_pools.push(SpawnPool::new(room_name, base_spawn_request.clone(), SpawnPoolOptions::default()));
        }

        for spawn_pool in spawn_pools.iter_mut() {
            // TODO Having a configurable lower and upper limit of the number of creeps and possibly
            //      total relevant body parts.
            // TODO Measuring number of idle creeps and trying to minimize their number while
            //      fulfilling all requests. To this end, keeping track of fulfillment of requests,
            //      how big is the backlog, etc.
            spawn_pool.with_spawned_creep(|creep_ref| async move {
                loop {
                    let mut maybe_withdraw_request = None;
                    let mut maybe_store_request = None;

                    with_haul_requests(room_name, |schedule| {
                        debug!(
                            "{} searching for withdraw/pickup and store requests.",
                            creep_ref.borrow().name
                        );
                        debug!("withdraw_requests: {:?}", schedule.withdraw_requests);
                        debug!("store_requests: {:?}", schedule.store_requests);

                        if schedule.withdraw_requests.is_empty() || schedule.store_requests.is_empty() {
                            return;
                        }

                        let creep_pos = u!(creep_ref.borrow_mut().pos());

                        let maybe_closest_withdraw_request_data = schedule
                            .withdraw_requests
                            .iter()
                            .filter_map(|(&id, request)| {
                                request.xy.map(|xy| (id, xy, xy.get_range_to(creep_pos)))
                            })
                            .min_by_key(|&(_, _, d)| d);

                        if let Some((closest_withdraw_request_id, withdraw_xy, _)) = maybe_closest_withdraw_request_data {
                            let maybe_closest_store_request_data = schedule
                                .store_requests
                                .iter()
                                .filter_map(|(&id, request)| request.xy.map(|xy| (id, xy.get_range_to(withdraw_xy))))
                                .min_by_key(|&(_, d)| d);

                            if let Some((closest_store_request_id, _)) = maybe_closest_store_request_data {
                                maybe_withdraw_request = schedule.withdraw_requests.remove(&closest_withdraw_request_id);
                                maybe_store_request = schedule.store_requests.remove(&closest_store_request_id);
                            }
                        }
                    });

                    if let Some(withdraw_request) = maybe_withdraw_request.take() {
                        let store_request = u!(maybe_store_request.take());

                        let result = fulfill_requests(&creep_ref, Some(withdraw_request), Some(store_request)).await;

                        if let Err(e) = result {
                            debug!("Error when hauling: {:?}.", e);
                            sleep(1).await;
                        }
                    } else {
                        sleep(1).await;
                    }
                }
            });
        }

        sleep(1).await;
    }
}

async fn fulfill_requests(
    creep_ref: &CreepRef,
    maybe_withdraw_request: Option<RawWithdrawRequest>,
    maybe_store_request: Option<RawStoreRequest>,
) -> Result<(), XiError> {
    if let Some(withdraw_request) = maybe_withdraw_request {
        let withdraw_travel_spec = TravelSpec {
            target: u!(withdraw_request.xy),
            range: 1,
        };

        // Creep may die on the way.
        travel(creep_ref, withdraw_travel_spec).await?;

        if withdraw_request.pickupable {
            pickup_when_able(creep_ref, withdraw_request.target).await?;
        } else {
            withdraw_when_able(creep_ref, withdraw_request.target, withdraw_request.resource_type, withdraw_request.amount).await?;
        }
    }

    if let Some(store_request) = maybe_store_request {
        let store_travel_spec = TravelSpec {
            target: u!(store_request.xy),
            range: 1,
        };

        match async {
            // Creep may die on the way.
            travel(creep_ref, store_travel_spec).await?;
            transfer_when_able(creep_ref, store_request.target, ResourceType::Energy, None).await?;
            Ok(())
        }.await {
            Err(XiError::CreepDead) => (),
            Err(_) => store_anywhere_or_drop(creep_ref).await?,
            Ok(()) => (),
        }
    }

    Ok(())
}

fn hauler_spawn_request(room_name: RoomName) -> SpawnRequest {
    // Prefer being spawned closer to the storage.
    let (preferred_spawns, body) = u!(with_room_state(room_name, |room_state| {
        let mut spawns = room_state
            .spawns
            .iter()
            .map(|spawn_data| {
                (
                    spawn_data.xy,
                    PreferredSpawn {
                        id: spawn_data.id,
                        directions: Vec::new(),
                        extra_cost: 0,
                    },
                )
            })
            .collect::<Vec<_>>();
        if let Some(storage_xy) = room_state
            .structures
            .get(&Storage)
            .and_then(|xys| xys.iter().next().cloned())
        {
            spawns.sort_by_key(|(spawn_xy, _)| spawn_xy.dist(storage_xy));
        }

        let preferred_spawns = spawns
            .into_iter()
            .map(|(_, preferred_spawn)| preferred_spawn)
            .collect::<Vec<_>>();

        let body = hauler_body(room_state);

        (preferred_spawns, body)
    }));

    let min_preferred_tick = game_tick();
    let max_preferred_tick = game_tick() + 1000;

    SpawnRequest {
        role: CreepRole::Hauler,
        body,
        priority: HAULER_SPAWN_PRIORITY,
        preferred_spawns,
        tick: (min_preferred_tick, max_preferred_tick),
    }
}

pub fn hauler_body(room_state: &RoomState) -> CreepBody {
    // TODO Instead of unwrap in such places, there should be a separate section for owned rooms that is guaranteed to be updated each tick.
    // TODO It seems double borrow doesnt work here.
    let spawn_energy = room_state.resources.spawn_energy;

    let parts = if spawn_energy >= 550 {
        repeat([Carry, Move]).take(5).flatten().collect::<Vec<_>>()
    } else {
        vec![Carry, Move, Carry, Move, Carry, Move]
    };

    CreepBody::new(parts)
}
