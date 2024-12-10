use log::{trace, warn};
use screeps::{ResourceType, RoomName, CREEP_RANGED_ACTION_RANGE};
use screeps::game::get_object_by_id_typed;
use crate::creeps::creep_role::CreepRole;
use crate::creeps::creep_body::CreepBody;
use crate::geometry::room_xy::RoomXYUtils;
use crate::hauling::requests::HaulRequest;
use crate::hauling::requests::HaulRequestKind::DepositRequest;
use crate::hauling::requests::HaulRequestTargetKind::CreepTarget;
use crate::hauling::scheduling_hauls::schedule_haul;
use crate::hauling::transfers::TransferStage::AfterAllTransfers;
use crate::kernel::sleep::sleep;
use crate::kernel::wait_until_some::wait_until_some;
use crate::priorities::BUILDER_SPAWN_PRIORITY;
use crate::room_states::room_states::with_room_state;
use crate::spawning::spawn_pool::{SpawnPool, SpawnPoolOptions};
use crate::spawning::spawn_schedule::{PreferredSpawn, SpawnRequest};
use crate::travel::travel::travel;
use crate::travel::travel_spec::TravelSpec;
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
                pos: spawn_data.xy.to_pos(room_name),
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
            // TODO Move the pool outside and just configure the number of creeps to zero when not needed.
            let (builders_required, builder_body) = wait_until_some(|| with_room_state(room_name, |room_state| {
                room_state
                    .eco_config
                    .as_ref()
                    .map(|config| {
                        (config.builders_required, config.builder_body.clone())
                    })
            }).flatten()).await;
            base_spawn_request.body = builder_body;
            let spawn_pool_options = SpawnPoolOptions::default()
                .target_number_of_creeps(builders_required);
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
                
                u!(spawn_pool.as_mut()).with_spawned_creeps(|creep_ref| async move {
                    let capacity = u!(creep_ref.borrow_mut().carry_capacity());
                    let creep_id = u!(creep_ref.borrow_mut().screeps_id());
                    let build_energy_consumption = creep_ref.borrow_mut().build_energy_consumption();
                    
                    // TODO After spawning the builder, making it pick up the energy from storage
                    //      if there is one.

                    // Travelling to the construction site.
                    let travel_spec = TravelSpec {
                        target: cs_data.xy.to_pos(room_name),
                        range: CREEP_RANGED_ACTION_RANGE,
                    };

                    if let Err(err) = travel(&creep_ref, travel_spec.clone()).await {
                        warn!("Builder could not reach its destination: {err}");
                        // Trying next tick (if the creep didn't die).
                        sleep(1).await;
                    }

                    let mut store_request = None;

                    // Building the construction site.
                    loop {
                        let cs = get_object_by_id_typed(&cs_data.id);
                        if cs.is_none() {
                            // The building is finished or the construction site stopped existing.
                            break;
                        }
                        
                        // This can only fail if the creep died, but then this process would be killed.
                        if u!(creep_ref.borrow_mut().used_capacity(Some(ResourceType::Energy), AfterAllTransfers)) >= build_energy_consumption {
                            creep_ref
                                .borrow_mut()
                                .build(u!(cs.as_ref()))
                                .warn_if_err("Failed to build the construction site");

                            // TODO Handle cancellation by drop (when creep dies).
                            store_request = None;
                        } else if store_request.is_none() {
                            // TODO Request the energy in advance.
                            let mut new_store_request = HaulRequest::new(
                                DepositRequest,
                                room_name,
                                ResourceType::Energy,
                                creep_id,
                                CreepTarget,
                                false,
                                creep_ref.borrow_mut().travel_state.pos
                            );
                            new_store_request.amount = capacity;
                            new_store_request.priority = Priority(30);
                            
                            store_request = Some(schedule_haul(new_store_request, store_request.take()));
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