use std::cmp::min;
use crate::creeps::creep_role::CreepRole;
use crate::kernel::sleep::sleep;
use crate::priorities::MINER_SPAWN_PRIORITY;
use crate::room_states::room_states::with_room_state;
use crate::creeps::creep_body::CreepBody;
use crate::travel::travel::travel;
use crate::u;
use crate::utils::result_utils::ResultUtils;
use log::{debug, warn};
use screeps::game::get_object_by_id_typed;
use screeps::look::ENERGY;
use screeps::{HasId, ResourceType, RoomName};
use crate::consts::FAR_FUTURE;
use crate::geometry::room_xy::RoomXYUtils;
use crate::hauling::requests::HaulRequest;
use crate::hauling::requests::HaulRequestKind::WithdrawRequest;
use crate::hauling::requests::HaulRequestTargetKind::PickupTarget;
use crate::hauling::scheduling_hauls::schedule_haul;
use crate::kernel::wait_until_some::wait_until_some;
use crate::room_states::utils::run_future_until_structures_change;
use crate::spawning::preferred_spawn::best_spawns;
use crate::spawning::spawn_pool::{SpawnPool, SpawnPoolOptions};
use crate::spawning::spawn_schedule::SpawnRequest;
use crate::travel::travel_spec::TravelSpec;
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

            let preferred_spawns = best_spawns(room_state, source_data.work_xy);

            // TODO
            // let best_spawn_xy = u!(room_state.spawns.first()).xy;
            // let best_spawn_pos = best_spawn_xy.to_pos(room_name);

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
        let target_rect_priority = Priority(220);
        let travel_spec = match mining_kind {
            MiningKind::DropMining => TravelSpec::new(
                source_data.xy.to_pos(room_name),
                1
            ).with_target_rect_priority(target_rect_priority),
            _ => TravelSpec::new(
                u!(source_data.work_xy).to_pos(room_name),
                0
            ).with_target_rect_priority(target_rect_priority),
        };

        let spawn_pool_options = SpawnPoolOptions::default()
            .travel_spec(Some(travel_spec.clone()));
        let mut spawn_pool = SpawnPool::new(room_name, base_spawn_request, spawn_pool_options);

        // TODO xi::kernel::kernel: Running P18-mine_source_W5N8_X14_Y13 (!198/P15).
        //      xi::room_states::utils: Structures changed. Killing the process P31.
        //      xi::kernel::kernel: Killed P31-loop_until_structures_change (!199/P18).
        //      xi::spawning::spawn_pool: Dropping a spawn pool element in W5N8 for Miner.
        //      Caught exception: RuntimeError: unreachable
        //      Stacktrace: RuntimeError: unreachable
        //          at std::panicking::rust_panic_with_hook::h5a293743c3d3b934 (wasm-function[1887]:199)
        //          at std::panicking::begin_panic_handler::{{closure}}::h6b29e7e43d527cf2 (wasm-function[2006]:175)
        //          at rust_begin_unwind (wasm-function[2668]:40)
        //          at core::panicking::panic_fmt::h05cda0e388c956d8 (wasm-function[2669]:38)
        //          at core::panicking::panic::h0b48879becc7f3f9 (wasm-function[2449]:61)
        //          at core::option::unwrap_failed::h0fa93bcd6e14296a (wasm-function[2859]:10)
        //          at xi::kernel::kernel::kernel::h539fd6b54557d319 (wasm-function[827]:679)
        //          at xi::kernel::kernel::kill_without_result_or_cleanup::h7ce2639ae986f34e (wasm-function[218]:28)
        //          at ::drop::he18f155e26fe6c72 (wasm-function[488]:974)
        //          at core::ptr::drop_in_place::h01dcf0c2e84148aa (wasm-function[1366]:95)
        run_future_until_structures_change(room_name, async move {
            loop {
                let (source_miners_required, miner_body, miner_spawn_priority) = wait_until_some(|| with_room_state(room_name, |room_state| {
                    room_state
                        .eco_config
                        .as_ref()
                        .map(|config| {
                            // Dividing the miners among sources.
                            let number_of_sources = room_state.sources.len();
                            let mut source_miners_required = config.miners_required / number_of_sources as u32;
                            // TODO Prioritize the source closest to spawn.
                            if (source_ix as u32) < config.miners_required - number_of_sources as u32 * source_miners_required {
                                source_miners_required += 1;
                            }
                            // The number of miners may not exceed the number of neighboring places to stand on.
                            source_miners_required = min(source_miners_required, room_state.sources[source_ix].drop_mining_xys.len() as u32);
                            (source_miners_required, config.miner_body.clone(), config.miner_spawn_priority)
                        })
                }).flatten()).await;
                spawn_pool.target_number_of_creeps = min(source_data.drop_mining_xys.len() as u32, source_miners_required);
                spawn_pool.base_spawn_request.body = miner_body;
                spawn_pool.base_spawn_request.priority = miner_spawn_priority;
                
                // Keeping a miner or multiple miners spawned and mining.
                spawn_pool.with_spawned_creeps(|creep_ref| {
                    let travel_spec = travel_spec.clone();
                    async move {
                        let miner = creep_ref.as_ref();
                        let energy_income = creep_ref.borrow().body.energy_harvest_power();

                        // Moving towards the location.
                        while let Err(err) = travel(&creep_ref, travel_spec.clone()).await {
                            warn!("Miner could not reach its destination: {err}.");
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
                                // TODO eco_stats.register_idle_creep(Miner);
                                sleep(source.ticks_to_regeneration().unwrap_or(1)).await;
                                continue;
                            }

                            // Transporting the energy in a way depending on room plan.
                            match mining_kind {
                                MiningKind::DropMining => {
                                    let creep_pos = creep_ref.borrow_mut().travel_state.pos;
                                    if let Some(dropped_energy) = u!(creep_pos.look_for(ENERGY)).first() {
                                        let amount = dropped_energy.amount();
                                        let mut new_pickup_request = HaulRequest::new(
                                            WithdrawRequest,
                                            room_name,
                                            ResourceType::Energy,
                                            dropped_energy.id(),
                                            PickupTarget,
                                            false,
                                            creep_pos
                                        );
                                        new_pickup_request.amount = amount;
                                        let decay = decay_per_tick(amount);
                                        new_pickup_request.change = energy_income as i32 - decay as i32;
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