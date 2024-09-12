use crate::creep::CreepRole;
use crate::hauling::{schedule_pickup, WithdrawRequest};
use crate::kernel::sleep::sleep;
use crate::priorities::MINER_SPAWN_PRIORITY;
use crate::resources::room_resources;
use crate::room_state::room_states::with_room_state;
use crate::spawn_pool::SpawnPool;
use crate::creep::CreepBody;
use crate::spawning::{PreferredSpawn, SpawnRequest};
use crate::travel::{predicted_travel_ticks, travel, TravelSpec};
use crate::u;
use crate::utils::return_code_utils::ReturnCodeUtils;
use log::warn;
use screeps::game::get_object_by_id_typed;
use screeps::look::ENERGY;
use screeps::Part::{Move, Work};
use screeps::{HasTypedId, Position, RoomName};
use std::cmp::min;

pub async fn mine_source(room_name: RoomName, source_ix: usize) {
    let mut structures_broadcast = u!(with_room_state(room_name, |room_state| {
        room_state.structures_broadcast.clone()
    }));

    loop {
        // Computing a schema for spawn request that will later have its tick intervals modified.
        // Also computing travel time for prespawning.
        let (base_spawn_request, source_data, travel_ticks, work_pos) = u!(with_room_state(room_name, |room_state| {
            let source_data = room_state.sources[source_ix];

            let work_xy = u!(source_data.work_xy);
            let work_pos = Position::new(work_xy.x, work_xy.y, room_name);
            // TODO container id in source_data
            // TODO link id in source_data (not necessarily xy)

            let body = miner_body(room_name);

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
                preferred_tick: (0, 0),
            };

            (base_spawn_request, source_data, travel_ticks, work_pos)
        }));

        // Travel spec for the miner. Will not change unless structures change.
        let travel_spec = TravelSpec {
            target: work_pos,
            range: 0,
        };

        let mut spawn_pool = SpawnPool::new(room_name, base_spawn_request, Some(travel_spec.clone()));

        loop {
            // When structures change, resetting everything.
            if structures_broadcast.check().is_none() {
                break;
            }

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
                    let ticks_to_live = miner.borrow().ticks_to_live();
                    let energy_per_tick = miner.borrow().energy_harvest_power();

                    // Moving towards the location.
                    if let Err(err) = travel(&creep_ref, travel_spec.clone()).await {
                        warn!("Miner could not reach its destination: {err}");
                        // Trying next tick (if the creep didn't die).
                        sleep(1).await;
                    }

                    let mut withdraw_request = None;

                    // Mining. We do not have to check that the miner exists, since it is done in with_spawned_creep.
                    loop {
                        let source = u!(get_object_by_id_typed(&source_data.id));
                        if source.energy() > 0 {
                            miner
                                .borrow()
                                .harvest(&source)
                                .to_bool_and_warn("Failed to mine the source");
                            sleep(1).await;
                        } else if miner.borrow().ticks_to_live() < source.ticks_to_regeneration() {
                            // If the miner does not exist by the time source regenerates, kill it.
                            miner.borrow().suicide();
                            // TODO Store the energy first.
                            break;
                        } else {
                            // The source is exhausted for now, so sleeping until it is regenerated.
                            sleep(source.ticks_to_regeneration()).await;
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
                            if let Some(dropped_energy) = work_pos.look_for(ENERGY).first() {
                                withdraw_request = Some(WithdrawRequest {
                                    room_name,
                                    target: dropped_energy.id(),
                                    xy: Some(work_pos),
                                    amount: dropped_energy.amount(),
                                    amount_per_tick: energy_per_tick,
                                    max_amount: min(1000, source.energy() + dropped_energy.amount()),
                                    priority: 100,
                                    preferred_tick: (0, 0),
                                });
                                // Ordering a hauler to get dropped energy.
                                let id = schedule_pickup(u!(withdraw_request));
                                // TODO updating it
                            }
                        }
                    }
                }
            });

            sleep(1).await;
        }
    }
}

fn miner_body(room_name: RoomName) -> CreepBody {
    let resources = room_resources(room_name);

    let parts = if resources.spawn_energy >= 550 {
        vec![Work, Work, Work, Work, Work, Move]
    } else {
        vec![Work, Work, Move, Move]
    };

    CreepBody::new(parts)
}
