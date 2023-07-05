use crate::config::SPAWN_SCHEDULE_TICKS;
use crate::creep::CreepRole;
use crate::creeps::find_idle_creep;
use crate::game_time::game_tick;
use crate::kernel::sleep::sleep;
use crate::priorities::MINER_SPAWN_PRIORITY;
use crate::resources::room_resources;
use crate::room_state::room_states::with_room_state;
use crate::spawning::{cancel_scheduled_creep, schedule_creep, CreepBody};
use crate::spawning::{PreferredSpawn, SpawnRequest};
use crate::travel::{predicted_travel_ticks, travel, TravelSpec};
use crate::u;
use crate::utils::return_code_utils::ReturnCodeUtils;
use log::warn;
use screeps::game::get_object_by_id_typed;
use screeps::Part::{Move, Work};
use screeps::{Position, RoomName, CREEP_LIFE_TIME};
use std::collections::VecDeque;

pub async fn mine_source(room_name: RoomName, source_ix: usize) {
    let mut structures_broadcast = u!(with_room_state(room_name, |room_state| {
        room_state.structures_broadcast.clone()
    }));
    let mut current_creep = None;
    let mut prespawned_creep = None;

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

        let mut spawn_conditions = VecDeque::new();

        // TODO On structures change, if spawns changed, manually resetting base spawn request, cancelling all requests and
        //      adding them again.
        // TODO The same if miner dies.
        // TODO Body should depend on max extension fill and also on current resources. Later, also on statistics about
        //      energy income, but this applies mostly before the storage is online.
        'with_base_spawn: while structures_broadcast.check().is_none() {
            // Getting the creep and scheduling missing spawn requests for the next
            // SPAWN_SCHEDULE_TICKS.
            while current_creep.is_none() {
                if let Some(miner) = find_idle_creep(
                    room_name,
                    base_spawn_request.role,
                    &base_spawn_request.body,
                    Some(work_pos),
                ) {
                    // At the beginning, we try to get an existing creep (in case of a restart).
                    current_creep.replace(miner);
                } else {
                    // If that fails, we schedule one if there is none scheduled and postpone
                    // scheduling others until the creep spawns and we know the initial tick.
                    if spawn_conditions.is_empty() {
                        let mut spawn_request = base_spawn_request.clone();
                        let min_preferred_tick = game_tick();
                        let max_preferred_tick = game_tick() + 1500; // TODO preferred now, no real limit
                        spawn_request.preferred_tick = (min_preferred_tick, max_preferred_tick);
                        if let Some(condition) = schedule_creep(room_name, spawn_request) {
                            spawn_conditions.push_back((min_preferred_tick, max_preferred_tick, condition));
                        }
                    }

                    // If the scheduling failed then trying again later.
                    if !spawn_conditions.is_empty() {
                        // There is no creep to work with, but at least one is scheduled, so waiting
                        // for it.
                        let (_, _, spawn_condition) = u!(spawn_conditions.front());
                        // If failed to spawn the creep, trying again next tick.
                        current_creep = spawn_condition.clone().await;
                    }
                }

                if structures_broadcast.check().is_some() {
                    break 'with_base_spawn;
                }

                if current_creep.is_none() {
                    sleep(1).await;
                }
            }

            // If we reached here, we have a creep.
            let miner = u!(current_creep.as_ref());
            let ticks_to_live = miner.borrow().ticks_to_live();

            // Moving towards the location.
            if let Err(err) = travel(miner, travel_spec.clone()).await {
                warn!("Miner could not reach its destination: {err}");
                if miner.borrow().exists() {
                    // To avoid infinite loop.
                    sleep(1).await;
                }
                current_creep = None;
                continue;
            }

            // Mining.
            while miner.borrow().exists() {
                // We create the schedule for subsequent creeps.
                let mut last_scheduled_creep_death_tick =
                    if let Some((last_spawn_min_preferred_tick, _, _)) = spawn_conditions.back() {
                        last_spawn_min_preferred_tick + CREEP_LIFE_TIME
                    } else {
                        game_tick() + miner.borrow().ticks_to_live()
                    };

                while last_scheduled_creep_death_tick <= game_tick() + SPAWN_SCHEDULE_TICKS {
                    let mut spawn_request = base_spawn_request.clone();
                    let min_preferred_tick = last_scheduled_creep_death_tick - travel_ticks;
                    let max_preferred_tick = min_preferred_tick + 200; // TODO
                    last_scheduled_creep_death_tick = min_preferred_tick + CREEP_LIFE_TIME;
                    spawn_request.preferred_tick = (min_preferred_tick, max_preferred_tick);
                    if let Some(condition) = schedule_creep(room_name, spawn_request) {
                        spawn_conditions.push_back((min_preferred_tick, max_preferred_tick, condition));
                    } else {
                        // If the scheduling failed then trying again later.
                        break;
                    }
                }

                // Handling prespawned creep.
                if prespawned_creep.is_none() {
                    if let Some((_, _, prespawned_creep_condition)) = spawn_conditions.front() {
                        if let Some(spawn_result) = prespawned_creep_condition.check() {
                            // Spawn finished. Might have failed.
                            // TODO Handle spawn failure.
                            prespawned_creep = spawn_result;

                            if let Some(next_miner) = prespawned_creep.as_ref() {
                                // Ignoring the result of travel since it will be reissued when
                                // the creep becomes the current miner.
                                // This should not create a congestion because the next miner should
                                // arrive exactly when the previous one dies.
                                travel(next_miner, travel_spec.clone());
                            }
                        }
                    }
                }

                let source = u!(get_object_by_id_typed(&source_data.id));
                if source.energy() > 0 {
                    miner
                        .borrow()
                        .harvest(&source)
                        .to_bool_and_warn("Failed to mine the source");
                    sleep(1).await;
                } else if miner.borrow().ticks_to_live() < source.ticks_to_regeneration() {
                    // If the miner will not exist by the time source regenerates, kill it.
                    miner.borrow().suicide();
                    current_creep = None;
                    break;
                } else {
                    // The source is exhausted for now, so sleeping until it is regenerated.
                    sleep(source.ticks_to_regeneration()).await;
                }

                // TODO
                // Transporting the energy in a way depending on room plan.
                // Ordering a hauler to get dropped energy.

                // Ordering a hauler to get energy from the container.

                // Storing the energy into the link and sending it if the next batch would not fit.

                if structures_broadcast.check().is_some() {
                    break 'with_base_spawn;
                }
            }
        }

        // Reinitializing the process if there are changes in structures.
        // Only doing it once the miner is dead or on some error (perhaps related to the reason
        // of the change, like destroyed link).
        while let Some((_, _, spawn_condition)) = spawn_conditions.pop_front() {
            cancel_scheduled_creep(room_name, spawn_condition);
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
