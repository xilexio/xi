use crate::creep::CreepRole;
use crate::creeps::{register_creep, CreepRef};
use crate::game_time::game_tick;
use crate::kernel::condition::Condition;
use crate::kernel::schedule;
use crate::kernel::sleep::sleep;
use crate::priorities::CREEP_REGISTRATION_PRIORITY;
use crate::room_state::room_states::with_room_state;
use crate::u;
use crate::utils::return_code_utils::ReturnCodeUtils;
use derive_more::Constructor;
use log::{debug, trace, warn};
use rustc_hash::FxHashMap;
use screeps::{game, Direction, ObjectId, Part, RoomName, SpawnOptions, StructureSpawn, CREEP_SPAWN_TIME};
use std::cell::RefCell;
use std::cmp::max;
use std::collections::BTreeMap;
use std::collections::Bound::Included;

thread_local! {
    static SPAWN_SCHEDULES: RefCell<FxHashMap<RoomName, RoomSpawnSchedule>> = RefCell::new(FxHashMap::default());
}

fn with_spawn_schedule<F, R>(room_name: RoomName, f: F) -> R
where
    F: FnOnce(&mut RoomSpawnSchedule) -> R,
{
    // TODO need scan data to create the schedule
    SPAWN_SCHEDULES.with(|states| {
        let mut borrowed_states = states.borrow_mut();
        let room_spawn_schedule = borrowed_states
            .entry(room_name)
            .or_insert_with(RoomSpawnSchedule::default);
        f(room_spawn_schedule)
    })
}

#[derive(Default)]
struct RoomSpawnSchedule {
    /// Map from ticks to spawn events at that tick.
    scheduled_spawns: FxHashMap<ObjectId<StructureSpawn>, BTreeMap<u32, SpawnEvent>>,
    // /// Map of spawn schedules for each creep role.
    // schedules: FxHashMap<CreepRole, SpawnSchedule>,
}

/// A scheduled spawn.
struct SpawnEvent {
    request: SpawnRequest,
    condition: Condition<Option<CreepRef>>,
}

/// A request with information needed to spawn the creep.
#[derive(Debug, Clone)]
pub struct SpawnRequest {
    pub role: CreepRole,
    pub body: CreepBody,
    pub priority: u8,
    /// Spawns in the order of preference. Must list all valid spawns and be ordered by `extra_cost`.
    pub preferred_spawn: Vec<PreferredSpawn>,
    pub preferred_tick: (u32, u32),
    // limit_tick: (u32, u32),
}

#[derive(Debug, Clone, Constructor)]
pub struct CreepBody {
    parts: Vec<Part>,
}

impl CreepBody {
    fn spawn_duration(&self) -> u32 {
        self.parts.len() as u32 * CREEP_SPAWN_TIME
    }
}

#[derive(Debug, Clone)]
pub struct PreferredSpawn {
    /// ID of the spawn to spawn from.
    pub id: ObjectId<StructureSpawn>,
    /// Allowed directions in which the creep should move from the spawn upon spawning.
    pub directions: Vec<Direction>,
    /// Extra energy cost incurred by selecting this spawn.
    pub extra_cost: u32,
}

/// Schedule a creep to be spawned within given tick and resource constraints.
pub fn schedule_creep(room_name: RoomName, request: SpawnRequest) -> Option<Condition<Option<CreepRef>>> {
    with_spawn_schedule(room_name, move |room_spawn_schedule| {
        let mut selected_spawn_data = None;

        for preferred_spawn in request.preferred_spawn.iter() {
            if let Some(spawn_schedule) = room_spawn_schedule.scheduled_spawns.get(&preferred_spawn.id) {
                if let Some(spawn_tick) = find_spawn_tick(&request, spawn_schedule) {
                    selected_spawn_data = Some((preferred_spawn.id, spawn_tick));
                    break;
                }
            } else {
                debug!("Ignoring nonexistent preferred spawn {}.", preferred_spawn.id);
            }
        }

        if let Some((spawn_id, spawn_tick)) = selected_spawn_data {
            let condition = Condition::default();

            let spawn_event = SpawnEvent {
                request,
                condition: condition.clone(),
            };

            // We already checked the spawn exists above.
            u!(room_spawn_schedule.scheduled_spawns.get_mut(&spawn_id)).insert(spawn_tick, spawn_event);

            Some(condition)
        } else {
            debug!(
                "Failed to spawn {:?} in {} because no spawn is free between ticks {} and {}.",
                request.role, room_name, request.preferred_tick.0, request.preferred_tick.1
            );
            None
        }
    })
}

fn find_spawn_tick(request: &SpawnRequest, schedule: &BTreeMap<u32, SpawnEvent>) -> Option<u32> {
    let spawn_duration = request.body.spawn_duration();
    let min_spawn_tick = max(game_tick(), request.preferred_tick.0 - spawn_duration);
    let max_spawn_tick = request.preferred_tick.1 - spawn_duration;

    // Trying min_spawn_tick and each tick after a creep finishes spawning.
    let mut spawn_tick = min_spawn_tick;
    let mut cursor = schedule.lower_bound(Included(&request.preferred_tick.0));
    loop {
        if let Some((other_spawn_tick, other_spawn_event)) = cursor.key_value() {
            if spawn_tick + spawn_duration < *other_spawn_tick {
                return Some(spawn_tick);
            } else {
                spawn_tick = *other_spawn_tick + other_spawn_event.request.body.spawn_duration() + 1;
                if spawn_tick > max_spawn_tick {
                    return None;
                }
                cursor.move_next();
            }
        } else {
            return Some(spawn_tick);
        }
    }
}

/// Cancels scheduled creep. Requires iterating over each scheduled spawn in a room. Does not cancel creeps that are
/// already spawning.
pub fn cancel_scheduled_creep(room_name: RoomName, condition: Condition<Option<CreepRef>>) {
    with_spawn_schedule(room_name, |room_spawn_schedule| {
        for (_, spawn_schedule) in room_spawn_schedule.scheduled_spawns.iter_mut() {
            let mut cancelled_spawn_tick = None;
            for (spawn_tick, spawn_event) in spawn_schedule.iter() {
                if spawn_event.condition.cid == condition.cid {
                    cancelled_spawn_tick = Some(*spawn_tick);
                    break;
                }
            }
            if let Some(spawn_tick) = cancelled_spawn_tick {
                spawn_schedule.remove(&spawn_tick);
                break;
            }
        }
    })
}

/// Issue the intents to spawn creeps in given room according to the schedule.
pub fn spawn_room_creeps(room_name: RoomName) {
    // let resources = room_resources(room_name);

    with_spawn_schedule(room_name, |room_spawn_schedule| {
        for (&spawn_id, spawn_schedule) in room_spawn_schedule.scheduled_spawns.iter_mut() {
            if let Some(entry) = spawn_schedule.first_entry() {
                if *entry.key() <= game_tick() {
                    let event = entry.remove();

                    debug!("Attempting to spawn {:?} in {}.", event.request.role, room_name);

                    if let Some(spawn) = game::get_object_by_id_typed(&spawn_id) {
                        // Nonexistent creeps are cleaned up next tick. This creep will exist the next tick, unless it
                        // fails to spawn.
                        let creep = register_creep(event.request.role);

                        let spawn_options = SpawnOptions::default();
                        if spawn
                            .spawn_creep_with_options(&event.request.body.parts, &creep.borrow().name, &spawn_options)
                            .to_bool_and_warn(&format!(
                                "Failed to spawn {:?} in spawn {} in {}",
                                event.request.role, spawn_id, room_name
                            ))
                        {
                            trace!("Spawning creep {:?} in {}.", event.request.role, room_name);
                            drop(schedule(
                                "creep_registration",
                                CREEP_REGISTRATION_PRIORITY,
                                async move {
                                    sleep(event.request.body.spawn_duration()).await;
                                    if creep.borrow().exists() {
                                        // Informing processes that the spawning succeeded.
                                        event.condition.signal(Some(creep));
                                    } else {
                                        warn!(
                                            "Failed to spawn {:?} in spawn {} in {}.",
                                            event.request.role, spawn_id, room_name
                                        );
                                        // TODO Reschedule if possible.
                                        // Informing processes that the spawning failed.
                                        event.condition.signal(None);
                                    }
                                },
                            ));
                        } else {
                            warn!(
                                "Failed to spawn {:?} in spawn {} in {}.",
                                event.request.role, spawn_id, room_name
                            );
                            // TODO Reschedule if possible.
                            // Informing processes that the spawning failed.
                            event.condition.signal(None);
                        }
                    } else {
                        warn!("Failed to find spawn {} in {}.", spawn_id, room_name);
                        // TODO Reschedule on another spawn if possible.
                        // Informing processes that the spawning failed.
                        event.condition.signal(None);
                    }
                }
            }
        }
        // while let Some((&event_tick, _)) = room_spawn_schedule.scheduled_spawns.first_key_value() {
        //     if event_tick <= game::time() {
        //         if let Some((_, spawn_event)) = room_spawn_schedule.scheduled_spawns.pop_from_first() {
        //             if let Some(spawn) = game::get_object_by_id_typed(&spawn_event) {
        //                 // TODO Decide on the body based on the role, energy available now, energy capacity and other factors.
        //                 //      If the energy is not available now, it should be rescheduled, potentially moving other, lower priority spawns for later.
        //                 //      Make some kind of statistics about energy levels and if there is not enough energy in the room or it exceeds maximum energy capacity,
        //                 //      try to make a more energy efficient/less costly version.
        //                 //      If it was rescheduled for other than energy reasons for some time, optionally bump the priority.
        //                 //      Introduce waiting for energy into the schedule. Move all ticks forward.
        //                 let body = creep_body(spawn_event.role, &resources);
        //                 let name = "xxx"; // TODO creep role and a counter with rotation
        //                                   // TODO energy_structures from fast filler to nearest extensions to furthest extensions
        //                                   // TODO directions
        //                 spawn.spawn_creep_with_options(&body, name, &SpawnOptions::default());
        //             } else {
        //                 // The spawn does not exist. It might have been destroyed.
        //                 // TODO
        //             }
        //         }
        //     }
        // }
    });
}

/// Should be called upon initialization and each time a spawn is destroyed or built.
pub fn update_spawn_list(room_name: RoomName) {
    debug!("Updating spawn list in room {}.", room_name);

    with_spawn_schedule(room_name, |room_spawn_schedule| {
        with_room_state(room_name, |room_state| {
            let mut scheduled_spawns = room_spawn_schedule
                .scheduled_spawns
                .drain()
                .collect::<FxHashMap<_, _>>();

            // This is supposed to be an owned room, so the unwrap is safe.
            // TODO use spawns instead
            for spawn_data in room_state.spawns.iter() {
                if let Some(spawn_schedule) = scheduled_spawns.remove(&spawn_data.id) {
                    // Old spawn schedule.
                    room_spawn_schedule
                        .scheduled_spawns
                        .insert(spawn_data.id, spawn_schedule);
                } else {
                    debug!("Registering a new spawn at {} in {}.", spawn_data.xy, room_name);
                    // New spawn schedule for a new spawn.
                    room_spawn_schedule
                        .scheduled_spawns
                        .insert(spawn_data.id, BTreeMap::default());
                }
            }

            // Removing spawn schedules of lost spawns.
            for (_, spawn_schedule) in scheduled_spawns {
                debug!("Unregistering a spawn in {}.", room_name);
                for (_, event) in spawn_schedule {
                    warn!(
                        "Failed to spawn {:?} in {} due to lost spawn.",
                        event.request.role, room_name
                    );
                    // TODO Reschedule on another spawn if possible.
                    // Informing processes that the spawning failed.
                    event.condition.signal(None);
                }
            }
        });
    });
}

// /// A schedule with information how many creeps of given type should be present in a room with priorities.
// struct SpawnSchedule {
//     room_name: RoomName,
//     role: CreepRole,
//     /// N-th element is the priority of spawning n-th creep. The length of the vector is how many creeps are supposed
//     /// to be spawned when the resources are plentiful.
//     priorities: Vec<u8>,
//     preferred_spawn: Option<ObjectId<StructureSpawn>>,
//     preferred_directions: Option<Vec<Direction>>,
//     delegate: Box<dyn FnMut(Creep)>,
// }
//
// /// Update the schedule to spawn creeps. Scheduling a creep to be spawned at given intervals and capacity.
// pub fn update_creep_spawn_schedule() {}
