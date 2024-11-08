use crate::creeps::creep::CreepRole;
use crate::hauling::schedule_pickup;
use crate::kernel::sleep::sleep;
use crate::priorities::MINER_SPAWN_PRIORITY;
use crate::room_state::room_states::with_room_state;
use crate::spawn_pool::{SpawnPool, SpawnPoolOptions};
use crate::creeps::creep::CreepBody;
use crate::spawning::{PreferredSpawn, SpawnRequest};
use crate::travel::{predicted_travel_ticks, travel, TravelSpec};
use crate::u;
use crate::utils::result_utils::ResultUtils;
use log::{debug, warn};
use screeps::game::get_object_by_id_typed;
use screeps::look::ENERGY;
use screeps::Part::{Move, Work};
use screeps::{HasId, Position, ResourceType, RoomName};
use crate::consts::FAR_FUTURE;
use crate::hauling::requests::WithdrawRequest;
use crate::room_state::RoomState;
use crate::room_state::utils::loop_until_structures_change;
use crate::utils::priority::Priority;

pub async fn mine_source(room_name: RoomName, source_ix: usize) {
    loop {
        // Computing a schema for spawn request that will later have its tick intervals modified.
        // Also computing travel time for prespawning.
        let (base_spawn_request, source_data, travel_ticks, work_pos) = u!(with_room_state(room_name, |room_state| {
            let body = miner_body(room_state);
            
            let source_data = room_state.sources[source_ix];

            let work_xy = u!(source_data.work_xy);
            let work_pos = Position::new(work_xy.x, work_xy.y, room_name);
            // TODO container id in source_data
            // TODO link id in source_data (not necessarily xy)

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

            // TODO
            let best_spawn_xy = u!(room_state.spawns.first()).xy;
            let best_spawn_pos = Position::new(best_spawn_xy.x, best_spawn_xy.y, room_name);

            let travel_ticks = predicted_travel_ticks(best_spawn_pos, work_pos, 1, 0, &body);

            let base_spawn_request = SpawnRequest {
                role: CreepRole::Miner,
                body,
                priority: MINER_SPAWN_PRIORITY,
                preferred_spawns,
                tick: (0, 0),
            };

            (base_spawn_request, source_data, travel_ticks, work_pos)
        }));

        // Travel spec for the miner. Will not change unless structures change.
        let travel_spec = TravelSpec {
            target: work_pos,
            range: 0,
        };

        let spawn_pool_options = SpawnPoolOptions::default().travel_spec(Some(travel_spec.clone()));
        let mut spawn_pool = SpawnPool::new(room_name, base_spawn_request, spawn_pool_options);

        loop_until_structures_change(room_name, 1, || {
            // TODO Body should depend on max extension fill and also on current resources. Later, also on statistics
            //      about energy income, but this applies mostly before the storage is online.
            // Keeping a miner spawned and mining with it.
            spawn_pool.with_spawned_creep(|creep_ref| {
                let travel_spec = travel_spec.clone();
                async move {
                    // TODO The problem is that we want to await travel, then await digging, etc., not check everything
                    //      each tick.
                    // TODO Make it so that this async will run as long as the creep exists and be killed when it does not.

                    let miner = creep_ref.as_ref();
                    let ticks_to_live = creep_ref.borrow_mut().ticks_to_live();
                    let energy_per_tick = creep_ref.borrow_mut().energy_harvest_power();
                    
                    // Moving towards the location.
                    if let Err(err) = travel(&creep_ref, travel_spec.clone()).await {
                        warn!("Miner could not reach its destination: {err}");
                        // Trying next tick (if the creep didn't die).
                        sleep(1).await;
                    }

                    let mut pickup_id = None;

                    // Mining. We do not have to check that the miner exists, since it is done in with_spawned_creep.
                    loop {
                        let source = u!(get_object_by_id_typed(&source_data.id));
                        if source.energy() > 0 {
                            creep_ref.borrow_mut()
                                .harvest(&source)
                                .warn_if_err("Failed to mine the source");
                            sleep(1).await;
                        } else if creep_ref.borrow_mut().ticks_to_live() < source.ticks_to_regeneration().unwrap_or(FAR_FUTURE) {
                            // If the miner does not exist by the time source regenerates, kill it.
                            debug!("Miner {} has insufficient ticks to live. Killing it.", miner.borrow().name);
                            creep_ref.borrow_mut().suicide().warn_if_err("Failed to kill the miner.");
                            // TODO Store the energy first.
                            break;
                        } else {
                            // The source is exhausted for now, so sleeping until it is regenerated.
                            sleep(source.ticks_to_regeneration().unwrap_or(1)).await;
                        }

                        // Transporting the energy in a way depending on room plan.
                        if let Some(link_id) = source_data.link_id {
                            // TODO
                            // Storing the energy into the link and sending it if the next batch would not fit.
                        } else if let Some(container_id) = source_data.container_id {
                            // TODO
                            // Ordering a hauler to get energy from the container.
                        } else {
                            // Drop mining.
                            if let Some(dropped_energy) = u!(work_pos.look_for(ENERGY)).first() {
                                let withdraw_request = WithdrawRequest {
                                    room_name,
                                    target: dropped_energy.id(),
                                    xy: Some(work_pos),
                                    resource_type: ResourceType::Energy,
                                    amount: None,
                                    // amount_per_tick: energy_per_tick,
                                    // max_amount: min(1000, source.energy() + dropped_energy.amount()),
                                    priority: Priority(100),
                                    // preferred_tick: (0, 0),
                                };
                                // Ordering a hauler to get dropped energy, updating the existing request.
                                pickup_id = Some(schedule_pickup(withdraw_request, pickup_id.take()));
                            }
                        }
                    }
                }
            });
            
            true
        }).await;
    }
}

fn miner_body(room_state: &RoomState) -> CreepBody {
    let spawn_energy = room_state.resources.spawn_energy;

    let parts = if spawn_energy >= 550 {
        vec![Work, Work, Work, Work, Work, Move]
    } else {
        vec![Work, Work, Move, Move]
    };

    CreepBody::new(parts)
}
