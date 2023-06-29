use std::collections::VecDeque;
use crate::creep::CreepRole;
use crate::kernel::sleep::sleep;
use crate::kernel::{kill_tree, schedule};
use crate::priorities::{MINER_SPAWN_PRIORITY, MINING_PRIORITY, ROOM_MAINTENANCE_PRIORITY, SPAWNING_CREEPS_PRIORITY};
use crate::resources::room_resources;
use crate::room_state::room_states::with_room_state;
use crate::spawning::{schedule_creep, spawn_room_creeps, update_spawn_list, CreepBody, PreferredSpawn, SpawnRequest};
use crate::u;
use log::debug;
use rustc_hash::{FxHashMap, FxHashSet};
use screeps::Part::{Carry, Move, Work};
use screeps::{game, Position, RoomName};
use std::iter::once;
use crate::config::SPAWN_SCHEDULE_TICKS;
use crate::game_time::game_tick;
use crate::travel::{travel, TravelSpec};

pub async fn maintain_rooms() {
    let mut room_processes = FxHashMap::default();

    loop {
        let mut lost_rooms = room_processes.keys().copied().collect::<FxHashSet<_>>();

        for room_name in game::rooms().keys() {
            lost_rooms.remove(&room_name);

            room_processes.entry(room_name).or_insert_with(|| {
                schedule(
                    &format!("room_process_{}", room_name),
                    ROOM_MAINTENANCE_PRIORITY - 2,
                    maintain_room(room_name),
                )
            });
        }

        for room_name in lost_rooms.into_iter() {
            let room_process = u!(room_processes.remove(&room_name));
            // TODO There are still many problems with kill_tree and releasing resources.
            kill_tree(room_process, ());
        }

        sleep(1).await;
    }
}

struct Miner {
    creep_name: Option<String>,
    creep_ticks_to_live: u32,
}

async fn maintain_room(room_name: RoomName) {
    // TODO the sources are constant, but link/container/drop is not

    // Collecting some constant data and waiting until the room state is set.
    let number_of_sources = loop {
        if let Some(number_of_sources) = with_room_state(room_name, |room_state| room_state.sources.len()) {
            break number_of_sources;
        } else {
            sleep(1).await;
        }
    };

    with_room_state(room_name, |room_state| {
        let structures_broadcast = room_state.structures_broadcast.clone();
        schedule(
            &format!("update_structures_{}", room_name),
            ROOM_MAINTENANCE_PRIORITY - 1,
            async move {
                loop {
                    update_spawn_list(room_name);

                    structures_broadcast.clone().await;
                    sleep(1).await;
                }
            },
        )
    });

    let miners: Vec<Option<u8>> = once(None).cycle().take(number_of_sources).collect();

    with_room_state(room_name, |room_state| {
        for (source_ix, source_data) in room_state.sources.iter().enumerate() {
            debug!("Setting up mining of {} in {}.", source_data.xy, room_name);
            drop(schedule(
                &format!("mine_source_{}_X{}_Y{}", room_name, source_data.xy.x, source_data.xy.y),
                MINING_PRIORITY,
                mine_source(room_name, source_ix),
            ));
            //         let miner = spawn(MINER).await;
            //         // TODO in background
            //         miner.mine().then(|res| {
            //             match res with {
            //                 Dead => ;
            //                 ...
            //             }
            //         })
            //         if dropped_resource > 100 {
            //             haul(resource).();
            //         }
        }
    });

    drop(schedule(
        &format!("spawn_creeps_{}", room_name),
        SPAWNING_CREEPS_PRIORITY,
        async move {
            loop {
                spawn_room_creeps(room_name);
                sleep(1).await;
            }
        },
    ));

    loop {
        debug!("Maintaining room {}.", room_name);

        sleep(1).await;
    }
}

async fn mine_source(room_name: RoomName, source_ix: usize) {
    let mut structures_broadcast = u!(with_room_state(room_name, |room_state| {
        room_state.structures_broadcast.clone()
    }));
    let mut spawn_conditions = VecDeque::new();
    let mut current_creep = None;
    // let mut prespawned_creep = None;

    loop {
        // A schema for spawn request that will later have its tick intervals modified.
        let base_spawn_request = u!(with_room_state(room_name, |room_state| SpawnRequest {
            role: CreepRole::Miner,
            body: miner_body(room_name),
            priority: MINER_SPAWN_PRIORITY,
            preferred_spawn: room_state
                .spawns
                .iter()
                .map(|spawn_data| PreferredSpawn {
                    id: spawn_data.id,
                    directions: Vec::new(),
                    extra_cost: 0
                })
                .collect(),
            preferred_tick: (0, 0),
        }));

        let source_data = u!(with_room_state(room_name, |room_state| {
            room_state.sources[source_ix]
        }));

        let work_xy = u!(source_data.work_xy);

        debug!("!!! {:?}", base_spawn_request.preferred_spawn);

        // TODO On structures change, if spawns changed, manually resetting base spawn request, cancelling all requests and
        //      adding them again.
        // TODO The same if miner dies.
        // TODO Body should depend on max extension fill and also on current resources. Later, also on statistics about
        //      energy income, but this applies mostly before the storage is online.
        loop {
            // Scheduling missing spawn requests for the next SPAWN_SCHEDULE_TICKS.
            while spawn_conditions.back().map(|(max_preferred_tick, _)| *max_preferred_tick <= SPAWN_SCHEDULE_TICKS).unwrap_or(true) {
                let mut spawn_request = base_spawn_request.clone();
                let min_preferred_tick = game_tick();
                let max_preferred_tick = game_tick() + 100; // TODO
                spawn_request.preferred_tick = (min_preferred_tick, max_preferred_tick);
                // If the scheduling failed then trying again later.
                if let Some(condition) = schedule_creep(room_name, spawn_request) {
                    spawn_conditions.push_back((max_preferred_tick, condition));
                }
            }

            // Getting a miner if the current one is dead.
            if let Some(miner) = current_creep.as_ref() {
                // Moving towards the location.
                // Owned room is expected to have `work_xy` set.
                let travel_spec = TravelSpec {
                    target: Position::new(work_xy.x, work_xy.y, room_name),
                    range: 0,
                };
                travel(miner, travel_spec).await;

                // Mining.

                // Transporting the energy in a way depending on room plan.
                // Ordering a hauler to get dropped energy.

                // Ordering a hauler to get energy from the container.

                // Storing the energy into the link and sending it if the next batch would not fit.

                // The source is exhausted for now, so sleeping until it is regenerated.

                // Sleeping for a single tick.
                sleep(1).await;
            } else if let Some((_, spawn_condition)) = spawn_conditions.pop_front() {
                // Assigning the current creep and immediately retrying.
                current_creep = spawn_condition.await;
            } else {
                // Retrying next tick.
                sleep(1).await;
            }

            // Reinitializing the process if there are changes in structures.
            if structures_broadcast.check().is_some() {
                break;
            }
        }
    }
}

fn miner_body(room_name: RoomName) -> CreepBody {
    let resources = room_resources(room_name);

    let parts = if resources.spawn_energy >= 600 {
        vec![Work, Work, Work, Work, Move, Move, Carry, Carry]
    } else if resources.spawn_energy >= 300 {
        vec![Work, Work, Move, Carry]
    } else {
        vec![Work, Move]
    };

    CreepBody::new(parts)
}
