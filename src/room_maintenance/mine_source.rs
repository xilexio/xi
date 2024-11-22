use crate::creeps::creep_role::CreepRole;
use crate::kernel::sleep::sleep;
use crate::priorities::MINER_SPAWN_PRIORITY;
use crate::room_states::room_states::with_room_state;
use crate::creeps::creep_body::CreepBody;
use crate::travel::{travel, TravelSpec};
use crate::u;
use crate::utils::result_utils::ResultUtils;
use log::{debug, warn};
use screeps::game::get_object_by_id_typed;
use screeps::look::ENERGY;
use screeps::{HasId, ResourceType, RoomName};
use crate::consts::FAR_FUTURE;
use crate::geometry::room_xy::RoomXYUtils;
use crate::hauling::requests::HaulRequest;
use crate::hauling::requests::HaulRequestKind::PickupRequest;
use crate::hauling::requests::RequestAmountChange::Increase;
use crate::hauling::scheduling_hauls::schedule_haul;
use crate::kernel::wait_until_some::wait_until_some;
use crate::room_states::utils::run_future_until_structures_change;
use crate::spawning::spawn_pool::{SpawnPool, SpawnPoolOptions};
use crate::spawning::spawn_schedule::{PreferredSpawn, SpawnRequest};
use crate::utils::priority::Priority;
use crate::utils::resource_decay::decay_per_tick;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum MiningKind {
    DropMining,
    ContainerMining,
    LinkMining,
}

pub async fn mine_source(room_name: RoomName, source_ix: usize) {
    loop {
        // Computing a template for spawn request that will later have its tick intervals modified.
        // Also computing travel spec. The working location (and hence travel spec) depends on the
        // kind of mining. Link and container mining are implemented only for single 5W+ creeps,
        // one per source in which the miner is staying in the planned work_xy.
        // Drop mining is implemented with dynamic travel for multiple creeps of any size to any
        // field neighboring the source.
        let (base_spawn_request, source_data) = u!(with_room_state(room_name, |room_state| {
            let source_data = room_state.sources[source_ix].clone();

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

            // TODO
            let best_spawn_xy = u!(room_state.spawns.first()).xy;
            let best_spawn_pos = best_spawn_xy.to_pos(room_name);

            // let travel_ticks = predicted_travel_ticks(best_spawn_pos, work_pos, 1, 0, &body);

            let base_spawn_request = SpawnRequest {
                role: CreepRole::Miner,
                body: CreepBody::empty(),
                priority: MINER_SPAWN_PRIORITY,
                preferred_spawns,
                tick: (0, 0),
            };

            (base_spawn_request, source_data)
        }));
        
        let mining_kind = match (source_data.link_id, source_data.container_id) {
            (Some(_), None) => MiningKind::LinkMining,
            (_, Some(_)) => MiningKind::ContainerMining,
            (None, None) => MiningKind::DropMining,
        };
        
        // Travel spec for the miner. Will not change unless structures change.
        let travel_spec = match mining_kind {
            MiningKind::DropMining => TravelSpec {
                target: source_data.xy.to_pos(room_name),
                range: 1,
            },
            _ => TravelSpec {
                target: u!(source_data.work_xy).to_pos(room_name),
                range: 0,
            },
        };

        let spawn_pool_options = SpawnPoolOptions::default()
            .travel_spec(Some(travel_spec.clone()));
        let mut spawn_pool = SpawnPool::new(room_name, base_spawn_request, spawn_pool_options);

        run_future_until_structures_change(room_name, async move {
            loop {
                let (miners_required_per_source, miner_body) = wait_until_some(|| with_room_state(room_name, |room_state| {
                    room_state
                        .eco_config
                        .as_ref()
                        .map(|config| {
                            (config.miners_required_per_source, config.miner_body.clone())
                        })
                }).flatten()).await;
                spawn_pool.target_number_of_creeps = miners_required_per_source;
                spawn_pool.base_spawn_request.body = miner_body;

                // Keeping a miner or multiple miners spawned and mining.
                spawn_pool.with_spawned_creeps(|creep_ref| {
                    let travel_spec = travel_spec.clone();
                    async move {
                        let miner = creep_ref.as_ref();

                        // Moving towards the location.
                        while let Err(err) = travel(&creep_ref, travel_spec.clone()).await {
                            warn!("Miner could not reach its destination: {err}");
                            // Trying next tick (if the creep didn't die).
                            sleep(1).await;
                        }

                        let mut pickup_request = None;

                        // Mining. We do not have to check that the miner exists, since it is done
                        // by the spawn pool.
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
                            match mining_kind {
                                MiningKind::DropMining => {
                                    let creep_pos = u!(creep_ref.borrow_mut().pos());
                                    if let Some(dropped_energy) = u!(creep_pos.look_for(ENERGY)).first() {
                                        let amount = dropped_energy.amount();
                                        let mut new_pickup_request = HaulRequest::new(
                                            PickupRequest,
                                            room_name,
                                            ResourceType::Energy,
                                            dropped_energy.id(),
                                            creep_pos
                                        );
                                        new_pickup_request.amount = amount;
                                        new_pickup_request.amount_change = Increase;
                                        new_pickup_request.decay = decay_per_tick(amount);
                                        new_pickup_request.priority = Priority(100);
    
                                        // Ordering a hauler to get dropped energy, updating the existing request.
                                        pickup_request = Some(schedule_haul(new_pickup_request, pickup_request.take()));
                                    }
                                }
                                MiningKind::ContainerMining => {
                                    let container_id = u!(source_data.container_id);
                                    // TODO
                                    // Ordering a hauler to get energy from the container.
                                }
                                MiningKind::LinkMining => {
                                    let link_id = u!(source_data.link_id);
                                    // TODO
                                    // Storing the energy into the link and sending it if the next batch would not fit.
                                }
                            }
                        }
                    }
                });
                
                sleep(1).await;
            }
        }).await;
    }
}