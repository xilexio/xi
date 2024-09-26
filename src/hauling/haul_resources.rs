use std::iter::repeat;
use std::vec;
use log::debug;
use screeps::game::get_object_by_id_erased;
use screeps::{Resource, ResourceType, ReturnCode, RoomName};
use screeps::Part::{Carry, Move};
use screeps::StructureType::Storage;
use wasm_bindgen::JsCast;
use crate::errors::XiError;
use crate::creeps::creep::{CreepBody, CreepRole};
use crate::creeps::CreepRef;
use crate::game_time::game_tick;
use crate::geometry::room_xy::RoomXYUtils;
use crate::hauling::requests::{with_haul_requests, RawStoreRequest, RawWithdrawRequest};
use crate::kernel::sleep::sleep;
use crate::priorities::HAULER_SPAWN_PRIORITY;
use crate::resources::room_resources;
use crate::room_state::room_states::with_room_state;
use crate::spawn_pool::{SpawnPool, SpawnPoolOptions};
use crate::spawning::{PreferredSpawn, SpawnRequest};
use crate::travel::{travel, TravelSpec};
use crate::u;

/// Execute hauling of resources of haulers assigned to given room.
/// Withdraw and store requests are registered in the system and the system assigns them to fre
/// haulers. One or more withdraw event is paired with one or more store events. There are special
/// withdraw and store events for the storage which may not be paired with one another.
pub async fn haul_resources(room_name: RoomName) {
    let base_spawn_request = u!(with_room_state(room_name, |room_state| {
        let body = hauler_body(room_name);

        // TODO
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
            preferred_tick: (0, 0),
        }
    }));
    
    let mut spawn_pool = SpawnPool::new(room_name, base_spawn_request, SpawnPoolOptions::default());
    
    loop {
        spawn_pool.with_spawned_creep(|creep_ref| async move {
            loop {
                let mut maybe_withdraw_request = None;
                let mut maybe_store_request = None;

                with_haul_requests(room_name, |schedule| {
                    debug!("{} searching for withdraw/pickup and store requests.", creep_ref.borrow().name);
                    debug!("{:?}", schedule.withdraw_requests);
                    debug!("{:?}", schedule.store_requests);

                    if schedule.withdraw_requests.is_empty() || schedule.store_requests.is_empty() {
                        return;
                    }

                    let creep_pos = creep_ref.borrow().pos();

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
                            .filter_map(|(&id, request)| {
                                request.xy.map(|xy| (id, xy.get_range_to(withdraw_xy)))
                            })
                            .min_by_key(|&(_, d)| d);

                        if let Some((closest_store_request_id, _)) = maybe_closest_store_request_data {
                            maybe_withdraw_request = schedule.withdraw_requests.remove(&closest_withdraw_request_id);
                            maybe_store_request = schedule.store_requests.remove(&closest_store_request_id);
                        }
                    }
                });

                if let Some(withdraw_request) = maybe_withdraw_request.take() {
                    let store_request = u!(maybe_store_request.take());

                    let result: Result<(), XiError> = (async {
                        let withdraw_travel_spec = TravelSpec {
                            target: u!(withdraw_request.xy),
                            range: 1,
                        };

                        let res = travel(&creep_ref, withdraw_travel_spec).await?;

                        if withdraw_request.pickupable {
                            if let Some(raw_resource) = get_object_by_id_erased(&withdraw_request.target) {
                                let resource = raw_resource.unchecked_into::<Resource>();
                                if creep_ref.borrow().pickup(&resource) != ReturnCode::Ok {
                                    return Err(XiError::CreepPickupFailed);
                                }
                            } else {
                                return Err(XiError::CreepPickupFailed);
                            }
                        } else {
                            // TODO
                        }

                        let store_travel_spec = TravelSpec {
                            target: u!(store_request.xy),
                            range: 1,
                        };

                        travel(&creep_ref, store_travel_spec).await?;
                        // TODO Minimum 1 tick of pause after withdraw even if in correct place.

                        if let Some(store_target) = get_object_by_id_erased(&store_request.target) {
                            creep_ref.borrow().unchecked_transfer(&store_target, ResourceType::Energy, None);
                        } else {
                            return Err(XiError::CreepTransferFailed);
                        }

                        Ok(())
                    }).await;

                    if let Err(e) = result {
                        debug!("Error when hauling: {:?}.", e);
                        sleep(1).await;
                    }
                } else {
                    sleep(1).await;
                }
            }
        });
        
        sleep(1).await;
    }
}

async fn fulfill_requests(creep_ref: CreepRef, withdraw_request: RawWithdrawRequest, store_request: RawStoreRequest) -> Result<(), XiError> {
    let withdraw_travel_spec = TravelSpec {
        target: u!(withdraw_request.xy),
        range: 1,
    };
    
    // Creep may die on the way.
    travel(&creep_ref, withdraw_travel_spec).await?;
    
    // We know that the creep is alive if the travel ended.
    if withdraw_request.pickupable {
        if let Some(raw_resource) = get_object_by_id_erased(&withdraw_request.target) {
            let resource = raw_resource.unchecked_into::<Resource>();
            if creep_ref.borrow().pickup(&resource) != ReturnCode::Ok {
                return Err(XiError::CreepPickupFailed);
            }
        } else {
            return Err(XiError::CreepPickupFailed);
        }
    } else {
        // TODO
    }
    
    let store_travel_spec = TravelSpec {
        target: u!(store_request.xy),
        range: 1,
    };
    
    travel(&creep_ref, store_travel_spec).await?;
    // TODO Minimum 1 tick of pause after withdraw even if in correct place.
    
    if let Some(store_target) = get_object_by_id_erased(&store_request.target) {
        creep_ref.borrow().unchecked_transfer(&store_target, ResourceType::Energy, None);
    } else {
        return Err(XiError::CreepTransferFailed);
    }
    
    Ok(())
}

fn hauler_spawn_request(room_name: RoomName) -> SpawnRequest {
    // Prefer being spawned closer to the storage.
    let preferred_spawns = u!(with_room_state(room_name, |room_state| {
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
        spawns
            .into_iter()
            .map(|(_, preferred_spawn)| preferred_spawn)
            .collect::<Vec<_>>()
    }));

    let min_preferred_tick = game_tick();
    let max_preferred_tick = game_tick() + 1000;

    SpawnRequest {
        role: CreepRole::Hauler,
        body: hauler_body(room_name),
        priority: HAULER_SPAWN_PRIORITY,
        preferred_spawns,
        preferred_tick: (min_preferred_tick, max_preferred_tick),
    }
}

fn hauler_body(room_name: RoomName) -> CreepBody {
    let resources = room_resources(room_name);

    let parts = if resources.spawn_energy >= 550 {
        repeat([Carry, Move]).take(5).flatten().collect::<Vec<_>>()
    } else {
        vec![Carry, Move, Carry, Move, Carry, Move]
    };

    CreepBody::new(parts)
}