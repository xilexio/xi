use crate::creeps::creeps::register_creep;
use crate::utils::game_tick::game_tick;
use crate::kernel::kernel::schedule;
use crate::kernel::sleep::sleep;
use crate::priorities::CREEP_REGISTRATION_PRIORITY;
use crate::room_states::room_states::with_room_state;
use crate::u;
use log::{debug, trace, warn};
use rustc_hash::{FxHashMap, FxHashSet};
use screeps::{game, ObjectId, RoomName, SpawnOptions, StructureSpawn};
use std::collections::Bound;
use crate::spawning::spawn_schedule::{with_spawn_schedule, PreferredSpawn, SpawnEvent};
use crate::utils::result_utils::ResultUtils;

const DEBUG: bool = true;

/// Issue the intents to spawn creeps in given room according to the schedule.
/// Handle the case with insufficient resources and other events preventing spawning.
pub fn spawn_room_creeps(room_name: RoomName) {
    // let resources = room_resources(room_name);

    // TODO update spawn_start_tick as now+1 when there is not enough energy

    let current_tick = game_tick();

    with_spawn_schedule(room_name, |room_spawn_schedule| {
        // Moving the spawn events for the current tick from future_spawns into current_spawns.
        if let Some(entry) = room_spawn_schedule.future_spawns.first_entry() {
            if *entry.key() <= current_tick {
                for (id, event) in entry.remove().drain() {
                    room_spawn_schedule.current_spawns.insert((event.request.priority, id), event);
                }
            }
        }

        // Issuing spawn intents from current_spawns as long as there are idle_spawns.
        let mut idle_spawns = room_spawn_schedule
            .spawns_in_progress
            .iter()
            .filter_map(|(&spawn_id, value)| value.is_none().then_some(spawn_id))
            .collect::<FxHashSet<_>>();
        
        if DEBUG {
            debug!(
                "Room {} has {} idle spawns and {} current spawn events. Current spawns:",
                room_name, idle_spawns.len(), room_spawn_schedule.current_spawns.len()
            );
            for current_spawn in room_spawn_schedule.current_spawns.values() {
                debug!(
                    "* {}, {}, {}",
                    current_spawn.request.role,
                    current_spawn.request.body,
                    current_spawn.request.priority
                );
            }
        }

        if !idle_spawns.is_empty() {
            // Iterate over the current spawns in priority (and ID) order, where the highest number
            // is the most urgent. When one with a preferred spawn that is idle is found,
            // execute it and remove the spawn from idle ones. Continue until there are no more
            // idle spawns or no more current spawn events. Also clean up all expired spawn events
            // on the way.
            // TODO For proper prioritization of spawns, check how much time there is left to spawn
            //      current highest priority one and let a lower priority one spawn first if the
            //      higher priority one will still make it in time and the lower priority one
            //      otherwise would not.
            let mut cursor = room_spawn_schedule.current_spawns.upper_bound_mut(Bound::Unbounded);
            while !idle_spawns.is_empty() {
                if let Some((_, event)) = cursor.prev() {
                    if event.request.tick.1 < current_tick + event.spawn_duration {
                        // The spawn request already expired or will not make it in time.
                        // Cancelling it.
                        let (_, event) = u!(cursor.remove_next());
                        debug!(
                            "Spawn request for {} expired in {} and was cancelled.",
                            event.request.role, room_name
                        );
                        event.promise.borrow_mut().cancelled = true;
                        continue;
                    }

                    let maybe_preferred_spawn = event
                        .request
                        .preferred_spawns
                        .iter()
                        .find(|PreferredSpawn { id, .. }| idle_spawns.contains(id))
                        .map(|preferred_spawn| preferred_spawn.id);
                    if let Some(preferred_spawn) = maybe_preferred_spawn {
                        idle_spawns.remove(&preferred_spawn);
                        if try_execute_spawn_event(room_name, preferred_spawn, event) {
                            let (_, event) = u!(cursor.remove_next());
                            room_spawn_schedule
                                .spawns_in_progress
                                .insert(preferred_spawn, Some(event));
                        }
                    }
                } else {
                    break;
                }
            }
        }
    });
}

fn try_execute_spawn_event(room_name: RoomName, spawn_id: ObjectId<StructureSpawn>, event: &SpawnEvent) -> bool {
    u!(with_room_state(room_name, |room_state| {
        if event.energy_cost > room_state.resources.spawn_energy {
            debug!("Not enough energy to spawn a {} creep in {} in spawn {}. {} is needed and {} is available.",
                event.request.role, room_name, spawn_id, event.energy_cost, room_state.resources.spawn_energy);
            return false;
        }

        debug!("Attempting to spawn {} in {}.", event.request.role, room_name);

        // TODO Cleanup spawn events beforehand if structures changed.
        let spawn = u!(game::get_object_by_id_typed(&spawn_id));

        // Nonexistent creeps are cleaned up next tick. This creep will exist the next tick, unless it
        // fails to spawn.
        let creep = register_creep(
            event.request.role,
            event.request.body.clone()
        );

        // Issuing the spawn intent.
        let spawn_options = SpawnOptions::default();
        let spawn_result = spawn
            .spawn_creep_with_options(&event.request.body.parts_vec(), &creep.borrow().name, &spawn_options);

        spawn_result.warn_if_err(&format!(
            "Failed to spawn {} in spawn {} in {}.",
            event.request.role,
            spawn_id,
            room_name
        ));
        if spawn_result.is_err() {
            return false;
        }

        {
            let mut promise = event.promise.borrow_mut();
            promise.spawn_id = Some(spawn_id);
            promise.spawn_end_tick = Some(game_tick() + event.spawn_duration);
        }

        // Updating the amount of available energy.
        room_state.resources.spawn_energy -= event.energy_cost;

        let promise = event.promise.clone();
        let spawn_duration = event.spawn_duration;
        let role = event.request.role;

        trace!("Spawning creep {} in {}.", event.request.role, room_name);
        schedule(
            "creep_registration",
            CREEP_REGISTRATION_PRIORITY,
            async move {
                sleep(spawn_duration).await;

                // Removing the spawn in progress event from the schedule.
                // Note that the room may have been lost, in which case there is no spawn schedule.
                with_spawn_schedule(room_name, |room_spawn_schedule| {
                    room_spawn_schedule
                        .spawns_in_progress
                        .get_mut(&spawn_id)
                        .map(|maybe_event| maybe_event.take());
                });

                // The creep may have died due to a destroyed spawn.
                if !creep.borrow().dead {
                    // Assigning the creep to the promise.
                    promise.borrow_mut().creep = Some(creep);
                } else {
                    // If the spawning does not succeed, logging it.
                    warn!(
                        "Failed to spawn {} in spawn {} in {}.",
                        role, spawn_id, room_name
                    );
                    // Informing processes that the spawning failed.
                    promise.borrow_mut().cancelled = true;
                    // TODO Reschedule if possible.
                }
            },
        );

        true
    }))
}

/// Fills spawn schedule's `spawns_in_progress` with spawns currently in the room.
/// Should be called upon initialization and each time a spawn is destroyed or built.
/// Cancels spawn promises when a spawn is destroyed. Does not change existing spawn requests'
/// preferred spawns, even if it included the destroyed one.
pub fn update_spawn_list(room_name: RoomName) {
    debug!("Updating spawn list in room {}.", room_name);

    with_spawn_schedule(room_name, |room_spawn_schedule| {
        with_room_state(room_name, |room_state| {
            let mut spawns_in_progress = room_spawn_schedule
                .spawns_in_progress
                .drain()
                .collect::<FxHashMap<_, _>>();

            for spawn_data in room_state.spawns.iter() {
                if let Some(spawn_schedule) = spawns_in_progress.remove(&spawn_data.id) {
                    // Old spawn schedule.
                    room_spawn_schedule
                        .spawns_in_progress
                        .insert(spawn_data.id, spawn_schedule);
                } else {
                    debug!("Registering a new spawn {} at {} in {}.", spawn_data.id, spawn_data.xy, room_name);
                    // New spawn schedule for a new spawn.
                    room_spawn_schedule
                        .spawns_in_progress
                        .insert(spawn_data.id, None);
                }
            }

            // Removing spawn schedules of lost spawns.
            for (spawn_id, maybe_spawn_event) in spawns_in_progress {
                debug!("Unregistering spawn {} in {}.", spawn_id, room_name);
                if let Some(event) = maybe_spawn_event {
                    warn!(
                        "Failed to spawn {} in {} due to a lost spawn.",
                        event.request.role, room_name
                    );
                    event.promise.borrow_mut().cancelled = true;
                }
            }
        });
    });
}