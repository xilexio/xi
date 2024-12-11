use crate::creeps::creep_role::CreepRole;
use crate::creeps::creeps::CreepRef;
use crate::errors::XiError;
use crate::utils::game_tick::game_tick;
use crate::geometry::room_xy::RoomXYUtils;
use crate::kernel::sleep::sleep;
use crate::priorities::HAULER_SPAWN_PRIORITY;
use crate::room_states::room_states::with_room_state;
use crate::travel::travel::travel;
use crate::u;
use log::debug;
use screeps::StructureType::Storage;
use screeps::RoomName;
use crate::creeps::actions::{pickup_when_able, transfer_when_able, withdraw_when_able};
use crate::creeps::creep_body::CreepBody;
use crate::hauling::requests::HaulRequestTargetKind::PickupTarget;
use crate::hauling::reserving_requests::{find_haul_requests, ReservedRequests};
use crate::hauling::store_anywhere_or_drop::store_anywhere_or_drop;
use crate::hauling::transfers::TransferStage::AfterAllTransfers;
use crate::kernel::wait_until_some::wait_until_some;
use crate::spawning::spawn_pool::{SpawnPool, SpawnPoolOptions};
use crate::spawning::spawn_schedule::{PreferredSpawn, SpawnRequest};
use crate::travel::travel_spec::TravelSpec;
use crate::utils::result_utils::ResultUtils;

const DEBUG: bool = true;

/// Execute hauling of resources of haulers assigned to given room.
/// Withdraw and store requests are registered in the system and the system assigns them to free
/// haulers. One or more withdraw event is paired with one or more store events. There are special
/// withdraw and store events for the storage which may not be paired with one another.
pub async fn haul_resources(room_name: RoomName) {
    let base_spawn_request = u!(with_room_state(room_name, |room_state| {
        // Any spawn is good.
        // TODO Remove directions reserved for the fast filler.
        let preferred_spawns = room_state
            .spawns
            .iter()
            .map(|spawn_data| PreferredSpawn {
                id: spawn_data.id,
                directions: Vec::new(),
                extra_cost: 0,
                pos: spawn_data.xy.to_pos(room_name),
            })
            .collect::<Vec<_>>();

        SpawnRequest {
            role: CreepRole::Hauler,
            body: CreepBody::empty(),
            priority: HAULER_SPAWN_PRIORITY,
            preferred_spawns,
            tick: (0, 0),
        }
    }));

    let mut spawn_pool = SpawnPool::new(room_name, base_spawn_request, SpawnPoolOptions::default());
    
    loop {
        let (haulers_required, hauler_body) = wait_until_some(|| with_room_state(room_name, |room_state| {
            // TODO
            if game_tick() % 10 == 4 {
                if let Some(eco_stats) = room_state.eco_stats.as_mut() {
                    eco_stats.haul_stats.add_sample(room_name);
                }
            }
            
            room_state
                .eco_config
                .as_ref()
                .map(|config| {
                    (config.haulers_required, config.hauler_body.clone())
                })
        }).flatten()).await;
        spawn_pool.target_number_of_creeps = haulers_required;
        spawn_pool.base_spawn_request.body = hauler_body;
        
        /* TODO This should not be needed. For now, let it break.
        with_haul_requests(room_name, |haul_requests| {
            haul_requests.withdraw_requests.retain(|_, request| {
                if request.borrow().kind != PickupRequest {
                    // TODO Not checking every single withdraw request as change in structures can
                    //      handle the rest.
                    true
                } else if erased_object_by_id(request.borrow().target).is_err() {
                    // Setting the request to not require any more resources.
                    // This combined with removing it from the map is exactly what cancelling
                    // request does.
                    request.borrow_mut().amount = 0;
                    false
                } else {
                    true
                }
            });
        });
        */

        // TODO Measuring number of idle creeps and trying to minimize their number while
        //      fulfilling all requests. To this end, keeping track of fulfillment of requests,
        //      how big is the backlog, etc.
        spawn_pool.with_spawned_creeps(|creep_ref| async move {
            let carry_capacity = u!(creep_ref.borrow_mut().carry_capacity());

            loop {
                let store = u!(creep_ref.borrow_mut().used_capacities(AfterAllTransfers));
                let pos = creep_ref.borrow_mut().travel_state.pos;
                let ttl = creep_ref.borrow_mut().ticks_to_live();

                debug!(
                    "{} searching for withdraw/pickup and store requests.",
                    creep_ref.borrow().name
                );
                
                let reserved_requests = find_haul_requests(
                    room_name,
                    &store,
                    pos,
                    carry_capacity,
                    ttl
                );

                if let Some(reserved_requests) = reserved_requests {
                    let result = fulfill_requests(&creep_ref, reserved_requests).await;

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

async fn fulfill_requests(creep_ref: &CreepRef, mut reserved_requests: ReservedRequests) -> Result<(), XiError> {
    // TODO This only works for singleton withdraw and store requests.
    if let Some(mut withdraw_request) = reserved_requests.withdraw_requests.pop() {
        let withdraw_travel_spec = TravelSpec {
            target: withdraw_request.request.borrow().pos,
            range: 1,
        };

        let result: Result<(), XiError> = async {
            // Creep may die on the way.
            travel(creep_ref, withdraw_travel_spec).await?;
            let target = withdraw_request.request.borrow().target;
            let resource_type = withdraw_request.request.borrow().resource_type;
            let limited_transfer = withdraw_request.request.borrow().limited_transfer;

            if withdraw_request.request.borrow().target_kind == PickupTarget {
                debug!(
                    "{} picking up {} {} from {}.",
                    creep_ref.borrow().name, withdraw_request.amount, resource_type, target
                );
                pickup_when_able(creep_ref, target).await?;
            } else {
                debug!(
                    "{} transferring {} {} from {}.",
                    creep_ref.borrow().name, withdraw_request.amount, resource_type, target
                );
                withdraw_when_able(creep_ref, target, resource_type, withdraw_request.amount, limited_transfer).await?;
            }
            
            withdraw_request.complete();
            
            Ok(())
        }.await;

        if result.is_err() {
            result.warn_if_err("Error while fulfilling a withdraw request");
            reserved_requests.withdraw_requests.push(withdraw_request);
            return result;
        }
    }

    if let Some(mut store_request) = reserved_requests.deposit_requests.pop() {
        let store_travel_spec = TravelSpec {
            target: store_request.request.borrow().pos,
            range: 1,
        };

        let result = async {
            // Creep may die on the way.
            travel(creep_ref, store_travel_spec).await?;
            let target = store_request.request.borrow().target;
            let resource_type = store_request.request.borrow().resource_type;
            let limited_transfer = store_request.request.borrow().limited_transfer;

            debug!(
                "{} storing {} {} in {}.",
                creep_ref.borrow().name, store_request.amount, resource_type, target
            );
            transfer_when_able(creep_ref, target, resource_type, store_request.amount, limited_transfer).await?;
            
            store_request.complete();
            
            Ok(())
        }.await;
        
        if result.is_err() {
            reserved_requests.deposit_requests.push(store_request);
        }
        
        match result {
            Err(XiError::CreepDead) => (),
            Err(_) => store_anywhere_or_drop(creep_ref).await?,
            Ok(()) => (),
        }
    }

    Ok(())
}

// TODO This function is not used. Extract closest spawn code from it and delete.
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
                        pos: spawn_data.xy.to_pos(room_name),
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

        let body = CreepBody::empty(); // TODO

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