use log::warn;
use screeps::{Position, ResourceType, RoomName};
use screeps::game::get_object_by_id_typed;
use crate::creeps::creep::{CreepBody, CreepRole};
use crate::hauling::issuing_requests::RequestAmountChange::NoChange;
use crate::hauling::issuing_requests::StoreRequest;
use crate::hauling::issuing_requests::schedule_store;
use crate::kernel::sleep::sleep;
use crate::kernel::wait_until_some::wait_until_some;
use crate::priorities::UPGRADER_SPAWN_PRIORITY;
use crate::room_state::room_states::with_room_state;
use crate::spawning::spawn_pool::{SpawnPool, SpawnPoolOptions};
use crate::spawning::spawn_schedule::{PreferredSpawn, SpawnRequest};
use crate::travel::{travel, TravelSpec};
use crate::u;
use crate::utils::priority::Priority;
use crate::utils::result_utils::ResultUtils;

pub async fn upgrade_controller(room_name: RoomName) {
    let (mut base_spawn_request, controller_id, work_pos) = u!(with_room_state(room_name, |room_state| {
        let controller_data = u!(room_state.controller);
        let work_xy = u!(controller_data.work_xy);

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

        let base_spawn_request = SpawnRequest {
            role: CreepRole::Upgrader,
            body: CreepBody::empty(),
            priority: UPGRADER_SPAWN_PRIORITY,
            preferred_spawns,
            tick: (0, 0),
        };

        (base_spawn_request, controller_data.id, Position::new(work_xy.x, work_xy.y, room_name))
    }));

    // Travel spec for the upgrader. Will not change unless structures change.
    let travel_spec = TravelSpec {
        target: work_pos,
        range: 0,
    };

    // TODO Handle prioritizing energy for the upgrading - always upgrade enough to prevent
    //      the room from downgrading, but only upgrade more if there is energy to spare.
    let spawn_pool_options = SpawnPoolOptions::default()
        .travel_spec(Some(travel_spec.clone()));
    let upgrader_body = wait_until_some(|| with_room_state(room_name, |room_state| {
            room_state
                .eco_config
                .as_ref()
                .map(|config| config.upgrader_body.clone())
        }).flatten()).await;
    base_spawn_request.body = upgrader_body;
    let mut spawn_pool = SpawnPool::new(room_name, base_spawn_request, spawn_pool_options);

    loop {
        spawn_pool.with_spawned_creep(|creep_ref| {
            let travel_spec = travel_spec.clone();
            async move {
                let capacity = u!(creep_ref.borrow_mut().store()).get_capacity(None);
                let creep_id = u!(creep_ref.borrow_mut().screeps_id());
                let upgrade_energy_consumption = u!(creep_ref.borrow_mut().upgrade_energy_consumption());

                if let Err(err) = travel(&creep_ref, travel_spec.clone()).await {
                    warn!("Upgrader could not reach its destination: {err}");
                    // Trying next tick (if the creep didn't die).
                    sleep(1).await;
                }

                let mut store_request_id = None;

                loop {
                    // This can only fail if the creep died, but then this process would be killed.
                    if u!(creep_ref.borrow_mut().store()).get_used_capacity(Some(ResourceType::Energy)) >= upgrade_energy_consumption {
                        let controller = u!(get_object_by_id_typed(&controller_id));
                        creep_ref
                            .borrow_mut()
                            .upgrade_controller(&controller)
                            .warn_if_err("Failed to upgrade the controller");

                        // TODO Handle cancellation by drop (when creep dies).
                        store_request_id = None;
                    } else if store_request_id.is_none() {
                        // TODO Request the energy in advance.
                        // TODO Use a container.
                        // TODO Use link.
                        let store_request = StoreRequest {
                            room_name,
                            target: creep_id,
                            resource_type: ResourceType::Energy,
                            pos: Some(work_pos),
                            amount: capacity,
                            amount_change: NoChange,
                            priority: Priority(40),
                        };
                        
                        store_request_id = Some(schedule_store(store_request, None));
                    }

                    sleep(1).await;
                }
            }
        });
        
        sleep(1).await;
    }
}