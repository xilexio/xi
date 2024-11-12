use crate::creeps::creep::{CreepBody, CreepRole};
use crate::creeps::CreepRef;
use crate::errors::XiError;
use crate::game_tick::game_tick;
use crate::geometry::room_xy::RoomXYUtils;
use crate::kernel::sleep::sleep;
use crate::priorities::HAULER_SPAWN_PRIORITY;
use crate::room_states::room_states::with_room_state;
use crate::travel::{travel, TravelSpec};
use crate::u;
use log::debug;
use screeps::StructureType::Storage;
use screeps::{ResourceType, RoomName};
use crate::creeps::actions::{pickup_when_able, transfer_when_able, withdraw_when_able};
use crate::hauling::matching_requests::{find_matching_requests, MatchingRequests};
use crate::hauling::store_anywhere_or_drop::store_anywhere_or_drop;
use crate::kernel::wait_until_some::wait_until_some;
use crate::spawning::spawn_pool::{SpawnPool, SpawnPoolOptions};
use crate::spawning::spawn_schedule::{PreferredSpawn, SpawnRequest};
use crate::utils::result_utils::ResultUtils;

/// Execute hauling of resources of haulers assigned to given room.
/// Withdraw and store requests are registered in the system and the system assigns them to fre
/// haulers. One or more withdraw event is paired with one or more store events. There are special
/// withdraw and store events for the storage which may not be paired with one another.
pub async fn haul_resources(room_name: RoomName) {
    let mut base_spawn_request = u!(with_room_state(room_name, |room_state| {
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
            body: CreepBody::empty(),
            priority: HAULER_SPAWN_PRIORITY,
            preferred_spawns,
            tick: (0, 0),
        }
    }));

    let mut spawn_pools = Vec::new();
    
    loop {
        let (haulers_required, hauler_body) = wait_until_some(|| with_room_state(room_name, |room_state| {
            room_state
                .eco_config
                .as_ref()
                .map(|config| {
                    (config.haulers_required, config.hauler_body.clone())
                })
        }).flatten()).await;

        // TODO Not spawning the replacement creep when the number of required haulers is lower than
        //      the number of existing spawn pools. Instead of maintaining a few spawn pools, this
        //      should be a single spawn pool handling multiple creeps.
        if spawn_pools.len() < haulers_required as usize {
            // TODO Update the spawn pool hauler body each time it changes.
            base_spawn_request.body = hauler_body;
            spawn_pools.push(SpawnPool::new(room_name, base_spawn_request.clone(), SpawnPoolOptions::default()));
        }

        for spawn_pool in spawn_pools.iter_mut() {
            // TODO Having a configurable lower and upper limit of the number of creeps and possibly
            //      total relevant body parts.
            // TODO Measuring number of idle creeps and trying to minimize their number while
            //      fulfilling all requests. To this end, keeping track of fulfillment of requests,
            //      how big is the backlog, etc.
            spawn_pool.with_spawned_creep(|creep_ref| async move {
                let carry_capacity = u!(creep_ref.borrow_mut().store()).get_capacity(None);

                loop {
                    debug!(
                        "{} searching for withdraw/pickup and store requests.",
                        creep_ref.borrow().name
                    );

                    let maybe_matching_requests = find_matching_requests(
                        room_name,
                        u!(creep_ref.borrow_mut().pos()),
                        carry_capacity
                    );

                    if let Some(matching_requests) = maybe_matching_requests {
                        let result = fulfill_requests(&creep_ref, matching_requests).await;

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

async fn fulfill_requests(creep_ref: &CreepRef, mut matching_requests: MatchingRequests) -> Result<(), XiError> {
    // TODO This only works for singleton withdraw and store requests.
    if let Some((request_id, withdraw_request)) = matching_requests.withdraw_requests.pop() {
        let withdraw_travel_spec = TravelSpec {
            target: u!(withdraw_request.pos),
            range: 1,
        };

        let result: Result<(), XiError> = async {
            // Creep may die on the way.
            travel(creep_ref, withdraw_travel_spec).await?;

            if withdraw_request.pickupable {
                pickup_when_able(creep_ref, withdraw_request.target).await?;
                matching_requests.withdraw_requests.pop();
            } else {
                withdraw_when_able(creep_ref, withdraw_request.target, withdraw_request.resource_type, Some(withdraw_request.amount)).await?;
                matching_requests.withdraw_requests.pop();
            }
            
            Ok(())
        }.await;
        
        
        if result.is_err() {
            result.warn_if_err("Error while fulfilling a withdraw request.");
            matching_requests.withdraw_requests.push((request_id, withdraw_request));
        }
    }

    if let Some((request_id, store_request)) = matching_requests.store_requests.pop() {
        let store_travel_spec = TravelSpec {
            target: u!(store_request.pos),
            range: 1,
        };

        let result = async {
            // Creep may die on the way.
            travel(creep_ref, store_travel_spec).await?;
            transfer_when_able(creep_ref, store_request.target, ResourceType::Energy, None).await?;
            matching_requests.store_requests.pop();
            Ok(())
        }.await;
        
        if result.is_err() {
            result.warn_if_err("Error while fulfilling a withdraw request.");
            matching_requests.store_requests.push((request_id, store_request));
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