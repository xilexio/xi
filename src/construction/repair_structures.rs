use screeps::{ResourceType, RoomName, CREEP_RANGED_ACTION_RANGE};
use crate::creeps::creep_role::CreepRole::Repairer;
use crate::geometry::room_xy::RoomXYUtils;
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
use crate::utils::get_object_by_id::structure_object_by_id;
use crate::utils::priority::Priority;
use crate::utils::result_utils::ResultUtils;

pub async fn repair_structures(room_name: RoomName) {
    let base_spawn_request = u!(with_room_state(room_name, |room_state| {
        generic_base_spawn_request(room_state, Repairer)
    }));

    let spawn_pool_options = SpawnPoolOptions::default();
    let mut spawn_pool = SpawnPool::new(room_name, base_spawn_request, spawn_pool_options);

    loop {
        let (repairers_required, repairer_body) = wait_until_some(|| with_room_state(room_name, |room_state| {
            room_state
                .eco_config
                .as_ref()
                .map(|config| {
                    (config.repairers_required, config.repairer_body.clone())
                })
        }).flatten()).await;
        spawn_pool.target_number_of_creeps = repairers_required;
        spawn_pool.base_spawn_request.body = repairer_body;
        
        spawn_pool.with_spawned_creeps(|creep_ref| async move {
            let capacity = u!(creep_ref.borrow_mut().carry_capacity());
            let creep_id = u!(creep_ref.borrow_mut().screeps_id());
            let repair_energy_consumption = creep_ref.borrow().body.repair_energy_usage();
            
            loop {
                let creep_pos = creep_ref.borrow().travel_state.pos;
                let best_repair_site = u!(with_room_state(room_name, |room_state| {
                    room_state.triaged_repair_sites.choose_repair_site(creep_pos.xy())
                }));
                
                if let Some(repair_site) = best_repair_site {
                    let travel_spec = TravelSpec::new(
                        repair_site.xy.to_pos(creep_pos.room_name()),
                        CREEP_RANGED_ACTION_RANGE
                    );
                    
                    if let Err(err) = travel(&creep_ref, travel_spec.clone()).await {
                        err.warn("Repairer could not reach its destination");
                        // Trying next tick (if the creep didn't die).
                        sleep(1).await;
                        continue;
                    }
        
                    let mut store_request = None;
        
                    loop {
                        // This can only fail if the creep died, but then this process would be killed.
                        let current_energy = u!(creep_ref.borrow_mut().used_capacity(Some(ResourceType::Energy), AfterAllTransfers));
                        if current_energy < capacity {
                            with_room_state(room_name, |room_state| {
                                if let Some(eco_stats) = room_state.eco_stats.as_mut() {
                                    eco_stats.register_idle_creep(Repairer, &creep_ref);
                                }
                            });
                            
                            let mut new_store_request = HaulRequest::new(
                                DepositRequest,
                                room_name,
                                ResourceType::Energy,
                                creep_id,
                                CreepTarget,
                                false,
                                creep_ref.borrow().travel_state.pos
                            );
                            new_store_request.amount = capacity;
                            new_store_request.priority = Priority(100);
                            new_store_request.change = repair_energy_consumption as i32;
                            new_store_request.max_amount = capacity;
        
                            store_request = Some(schedule_haul(new_store_request, store_request.take()));
                        } else {
                            store_request = None;
                        }
                        
                        // TODO Does this current_energy work or does it need to be one before transfers?
                        if current_energy >= repair_energy_consumption {
                            match structure_object_by_id(repair_site.id) {
                                Ok(target) => {
                                    let structure_obj = target.as_structure();
                                    if structure_obj.hits() == structure_obj.hits_max() {
                                        // Structure is already fully repaired. Remove it from
                                        // the list and finding a new one.
                                        with_room_state(room_name, |room_state| {
                                            room_state.triaged_repair_sites.remove_repair_site(repair_site.id);
                                        });
                                        break;
                                    }
                                    
                                    creep_ref
                                        .borrow_mut()
                                        .repair(u!(target.as_repairable()))
                                        .warn_if_err("Failed to repair the structure");
                                }
                                Err(e) => {
                                    e.warn(&format!(
                                        "Failed to repair {} {}",
                                        repair_site.structure_type, repair_site.id
                                    ));
                                }
                            }
                        }
        
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