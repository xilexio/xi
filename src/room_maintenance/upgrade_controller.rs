use log::warn;
use screeps::{ResourceType, RoomName, CREEP_RANGED_ACTION_RANGE};
use screeps::game::get_object_by_id_typed;
use crate::creeps::creep_body::CreepBody;
use crate::creeps::creep_role::CreepRole::Upgrader;
use crate::geometry::room_xy::RoomXYUtils;
use crate::hauling::requests::HaulRequest;
use crate::hauling::requests::HaulRequestKind::DepositRequest;
use crate::hauling::requests::HaulRequestTargetKind::CreepTarget;
use crate::hauling::scheduling_hauls::schedule_haul;
use crate::hauling::transfers::TransferStage::AfterAllTransfers;
use crate::kernel::sleep::sleep;
use crate::kernel::wait_until_some::wait_until_some;
use crate::priorities::UPGRADER_SPAWN_PRIORITY;
use crate::room_states::room_states::with_room_state;
use crate::spawning::preferred_spawn::best_spawns;
use crate::spawning::spawn_pool::{SpawnPool, SpawnPoolOptions};
use crate::spawning::spawn_schedule::SpawnRequest;
use crate::travel::travel::travel;
use crate::travel::travel_spec::TravelSpec;
use crate::u;
use crate::utils::priority::Priority;
use crate::utils::result_utils::ResultUtils;

pub async fn upgrade_controller(room_name: RoomName) {
    let (base_spawn_request, controller_id, work_pos, controller_pos) = u!(with_room_state(room_name, |room_state| {
        let controller_data = u!(room_state.controller);
        let work_xy = u!(controller_data.work_xy);

        // TODO
        let preferred_spawns = best_spawns(room_state, Some(work_xy));

        let base_spawn_request = SpawnRequest {
            role: Upgrader,
            body: CreepBody::empty(),
            priority: UPGRADER_SPAWN_PRIORITY,
            preferred_spawns,
            tick: (0, 0),
        };

        (base_spawn_request, controller_data.id, work_xy.to_pos(room_name), controller_data.xy.to_pos(room_name))
    }));

    // Travel spec for the upgrader. Will not change unless structures change.
    // TODO When link is present - around the link.
    //      Otherwise - around or on the container unless it is too far.
    //      It is okay to be next to container on low RCL.
    //      When under siege, don't be on unprotected places.
    //let travel_spec = TravelSpec::new(work_pos, 1);
    let travel_spec = TravelSpec::new(controller_pos, CREEP_RANGED_ACTION_RANGE);

    // TODO Handle prioritizing energy for the upgrading - always upgrade enough to prevent
    //      the room from downgrading, but only upgrade more if there is energy to spare.
    let spawn_pool_options = SpawnPoolOptions::default()
        .travel_spec(Some(travel_spec.clone()));
    let mut spawn_pool = SpawnPool::new(room_name, base_spawn_request, spawn_pool_options);

    loop {
        let (upgraders_required, upgrader_body) = wait_until_some(|| with_room_state(room_name, |room_state| {
            room_state
                .eco_config
                .as_ref()
                .map(|config| {
                    (config.upgraders_required, config.upgrader_body.clone())
                })
        }).flatten()).await;
        spawn_pool.target_number_of_creeps = upgraders_required;
        spawn_pool.base_spawn_request.body = upgrader_body;
        
        spawn_pool.with_spawned_creeps(|creep_ref| {
            let travel_spec = travel_spec.clone();
            async move {
                let capacity = u!(creep_ref.borrow_mut().carry_capacity());
                let creep_id = u!(creep_ref.borrow_mut().screeps_id());
                let upgrade_energy_consumption = creep_ref.borrow_mut().upgrade_energy_consumption();

                // TODO A way to await travel and ignore errors forever since there isn't anything
                //      that can be done outside of suicide. Similarly with other creeps.
                if let Err(err) = travel(&creep_ref, travel_spec.clone()).await {
                    warn!("Upgrader could not reach its destination: {err}.");
                    // Trying next tick (if the creep didn't die).
                    sleep(1).await;
                    // TODO Missing loop.
                }

                let mut store_request = None;

                loop {
                    // This can only fail if the creep died, but then this process would be killed.
                    let current_energy = u!(creep_ref.borrow_mut().used_capacity(Some(ResourceType::Energy), AfterAllTransfers));
                    if current_energy < capacity {
                        with_room_state(room_name, |room_state| {
                            if let Some(eco_stats) = room_state.eco_stats.as_mut() {
                                eco_stats.register_idle_creep(Upgrader, &creep_ref);
                            }
                        });

                        // TODO Use a container.
                        // TODO Use link.
                        let mut new_store_request = HaulRequest::new(
                            DepositRequest,
                            room_name,
                            ResourceType::Energy,
                            creep_id,
                            CreepTarget,
                            false,
                            work_pos
                        );
                        new_store_request.amount = capacity;
                        new_store_request.priority = Priority(40);
                        new_store_request.change = upgrade_energy_consumption as i32;
                        new_store_request.max_amount = capacity;

                        store_request = Some(schedule_haul(new_store_request, store_request.take()));
                    } else {
                        store_request = None;
                    }

                    // TODO Does this current_energy work or does it need to be one before transfers?
                    if current_energy >= upgrade_energy_consumption {
                        let controller = u!(get_object_by_id_typed(&controller_id));
                        creep_ref
                            .borrow_mut()
                            .upgrade_controller(&controller)
                            .warn_if_err("Failed to upgrade the controller");
                    }

                    sleep(1).await;
                }
            }
        });
        
        sleep(1).await;
    }
}