use log::{trace, warn};
use screeps::{Position, ResourceType, RoomName, CREEP_RANGED_ACTION_RANGE};
use screeps::game::get_object_by_id_typed;
use crate::creeps::creep::{CreepBody, CreepRole};
use crate::hauling::issuing_requests::RequestAmountChange::NoChange;
use crate::hauling::issuing_requests::StoreRequest;
use crate::hauling::issuing_requests::schedule_store;
use crate::kernel::sleep::sleep;
use crate::kernel::wait_until_some::wait_until_some;
use crate::priorities::BUILDER_SPAWN_PRIORITY;
use crate::room_states::room_states::with_room_state;
use crate::spawning::spawn_pool::{SpawnPool, SpawnPoolOptions};
use crate::spawning::spawn_schedule::{PreferredSpawn, SpawnRequest};
use crate::travel::{travel, TravelSpec};
use crate::u;
use crate::utils::priority::Priority;
use crate::utils::result_utils::ResultUtils;

pub async fn build_structures(room_name: RoomName) {
    let mut base_spawn_request = u!(with_room_state(room_name, |room_state| {
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
            role: CreepRole::Builder,
            body: CreepBody::empty(),
            priority: BUILDER_SPAWN_PRIORITY,
            preferred_spawns,
            tick: (0, 0),
        }
    }));

    // TODO Handle prioritizing energy for the upgrading - always upgrade enough to prevent
    //      the room from downgrading, but only upgrade more if there is energy to spare.
    let spawn_pool_options = SpawnPoolOptions::default();

    loop {
        // TODO pick construction site with highest priority
        // TODO spawn a builder
        // TODO send a builder to build it
        
        let cs_data = u!(with_room_state(room_name, |room_state| {
            if room_state.construction_site_queue.is_empty() {
                trace!("Nothing to build in {}.", room_name);
                None
            } else {
                trace!(
                    "Building the following structures in {}: {:?}.",
                    room_name, room_state.construction_site_queue
                );
                room_state.construction_site_queue.first().cloned()
            }
        }));

        if let Some(cs_data) = cs_data {
            // Initializing the spawn pool.
            let builder_body = wait_until_some(|| with_room_state(room_name, |room_state| {
                room_state
                    .eco_config
                    .as_ref()
                    .map(|config| config.builder_body.clone())
            }).flatten()).await;
            base_spawn_request.body = builder_body;
            let mut spawn_pool = Some(SpawnPool::new(room_name, base_spawn_request.clone(), spawn_pool_options.clone()));

            loop {
                let top_priority_cs_data_correct = u!(with_room_state(room_name, |room_state| {
                    room_state
                    .construction_site_queue
                    .first()
                    .map(|current_cs_data| current_cs_data.id == cs_data.id)
                    .unwrap_or(false)
                }));
                if !top_priority_cs_data_correct {
                    trace!(
                        "Current top priority construction site does not match the {} being build. Restarting the loop.",
                        cs_data.structure_type
                    );
                    // This also drops the spawn pool, thus releasing the reserved builder creep.
                    break;
                }
                
                u!(spawn_pool.as_mut()).with_spawned_creep(|creep_ref| async move {
                    let capacity = u!(creep_ref.borrow_mut().store()).get_capacity(None);
                    let creep_id = u!(creep_ref.borrow_mut().screeps_id());
                    let build_energy_consumption = u!(creep_ref.borrow_mut().build_energy_consumption());
                    
                    // TODO After spawning the builder, making it pick up the energy from storage
                    //      if there is one.

                    // Travelling to the construction site.
                    let travel_spec = TravelSpec {
                        target: Position::new(cs_data.xy.x, cs_data.xy.y, room_name),
                        range: CREEP_RANGED_ACTION_RANGE,
                    };

                    if let Err(err) = travel(&creep_ref, travel_spec.clone()).await {
                        warn!("Builder could not reach its destination: {err}");
                        // Trying next tick (if the creep didn't die).
                        sleep(1).await;
                    }

                    let mut store_request_id = None;

                    // Building the construction site.
                    loop {
                        let cs = get_object_by_id_typed(&cs_data.id);
                        if cs.is_none() {
                            // The building is finished or the construction site stopped existing.
                            break;
                        }
                        
                        // This can only fail if the creep died, but then this process would be killed.
                        if u!(creep_ref.borrow_mut().store()).get_used_capacity(Some(ResourceType::Energy)) >= build_energy_consumption {
                            creep_ref
                                .borrow_mut()
                                .build(u!(cs.as_ref()))
                                .warn_if_err("Failed to build the construction site");

                            // TODO Handle cancellation by drop (when creep dies).
                            store_request_id = None;
                        } else if store_request_id.is_none() {
                            // TODO Request the energy in advance.
                            let store_request = StoreRequest {
                                room_name,
                                target: creep_id,
                                resource_type: ResourceType::Energy,
                                pos: Some(u!(creep_ref.borrow_mut().pos())),
                                amount: capacity,
                                amount_change: NoChange,
                                priority: Priority(30),
                            };
                            
                            store_request_id = Some(schedule_store(store_request, None));
                        }

                        sleep(1).await;
                    }
                });
                
                sleep(1).await;
            }
        } else {
            sleep(10).await;
        }
    }
}