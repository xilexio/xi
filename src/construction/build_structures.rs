use log::{trace, warn};
use screeps::{ResourceType, RoomName, CREEP_RANGED_ACTION_RANGE};
use screeps::game::get_object_by_id_typed;
use crate::creeps::creep_role::CreepRole::Builder;
use crate::geometry::position_utils::PositionUtils;
use crate::hauling::requests::HaulRequest;
use crate::hauling::requests::HaulRequestKind::DepositRequest;
use crate::hauling::requests::HaulRequestTargetKind::CreepTarget;
use crate::hauling::scheduling_hauls::schedule_haul;
use crate::hauling::transfers::TransferStage::AfterAllTransfers;
use crate::kernel::sleep::sleep;
use crate::kernel::wait_until_some::wait_until_some;
use crate::room_states::room_states::with_room_state;
use crate::spawning::spawn_pool::{SpawnPool, SpawnPoolOptions};
use crate::spawning::spawn_schedule::generic_base_spawn_request;
use crate::travel::travel::travel;
use crate::travel::travel_spec::TravelSpec;
use crate::u;
use crate::utils::priority::Priority;
use crate::utils::result_utils::ResultUtils;

pub async fn build_structures(room_name: RoomName) {
    let base_spawn_request = u!(with_room_state(room_name, |room_state| {
        // TODO Maybe modify it later to the closest spawn to current construction site?
        generic_base_spawn_request(room_state, Builder)
    }));

    // TODO Handle prioritizing energy for the upgrading - always upgrade enough to prevent
    //      the room from downgrading, but only upgrade more if there is energy to spare.
    loop {
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
            let travel_spec = TravelSpec::new(cs_data.pos, CREEP_RANGED_ACTION_RANGE);

            let spawn_pool_options = SpawnPoolOptions::default()
                .travel_spec(Some(travel_spec.clone()));
            let mut spawn_pool = SpawnPool::new(room_name, base_spawn_request.clone(), spawn_pool_options);

            loop {
                let (top_priority_cs_data_correct, (builders_required, builder_body)) = wait_until_some(|| with_room_state(room_name, |room_state| {
                    Some((
                        room_state
                            .construction_site_queue
                            .first()
                            .map(|current_cs_data| current_cs_data.id == cs_data.id)?,
                        room_state
                            .eco_config
                            .as_ref()
                            .map(|config| {
                                (config.builders_required, config.builder_body.clone())
                            })?
                    ))
                }).flatten()).await;
                spawn_pool.target_number_of_creeps = builders_required;
                spawn_pool.base_spawn_request.body = builder_body;

                if !top_priority_cs_data_correct {
                    trace!(
                        "Current top priority construction site does not match the {} being build. Restarting the loop.",
                        cs_data.structure_type
                    );
                    // This also drops the spawn pool, thus releasing the reserved builder creep.
                    break;
                }

                trace!(
                    "Building {} at {} with {} creeps.",
                    cs_data.structure_type, cs_data.pos.f(), builders_required
                );

                spawn_pool.with_spawned_creeps(|creep_ref| {
                    let travel_spec = travel_spec.clone();
                    async move {
                        let capacity = u!(creep_ref.borrow_mut().carry_capacity());
                        let creep_id = u!(creep_ref.borrow_mut().screeps_id());
                        let build_energy_consumption = creep_ref.borrow_mut().build_energy_consumption();

                        // TODO After spawning the builder, making it pick up the energy from storage
                        //      if there is one.

                        // Travelling to the construction site.
                        if let Err(err) = travel(&creep_ref, travel_spec.clone()).await {
                            warn!("Builder could not reach its destination: {err}.");
                            // Trying next tick (if the creep didn't die).
                            sleep(1).await;
                        }

                        let mut store_request = None;

                        // Building the construction site.
                        loop {
                            let cs = get_object_by_id_typed(&cs_data.id);
                            if cs.is_none() {
                                // The building is finished or the construction site stopped existing.
                                // This future runs after the build_structures future, but this can run
                                // between ticks where construction sites are recreated.
                                break;
                            }

                            let current_energy = u!(creep_ref.borrow_mut().used_capacity(Some(ResourceType::Energy), AfterAllTransfers));

                            if current_energy < capacity {
                                with_room_state(room_name, |room_state| {
                                    if let Some(eco_stats) = room_state.eco_stats.as_mut() {
                                        eco_stats.register_idle_creep(Builder, &creep_ref);
                                    }
                                });

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
                                new_store_request.change = build_energy_consumption as i32;
                                new_store_request.max_amount = capacity;

                                store_request = Some(schedule_haul(new_store_request, store_request.take()));
                            } else {
                                store_request = None;
                            }

                            // This can only fail if the creep died, but then this process would be killed.
                            // TODO Does this current_energy work or does it need to be one before transfers?
                            if current_energy >= build_energy_consumption {
                                creep_ref
                                    .borrow_mut()
                                    .build(u!(cs.as_ref()))
                                    .warn_if_err("Failed to build the construction site");
                            }

                            sleep(1).await;
                        }
                    }
                });
                
                sleep(1).await;
            }
        } else {
            sleep(10).await;
        }
    }
}