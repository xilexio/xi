use crate::creep::{Creep, CreepRole};
use crate::game_time::game_tick;
use crate::resources::{room_resources, RoomResources};
use crate::utils::return_code_utils::ReturnCodeUtils;
use log::{debug, warn};
use rustc_hash::FxHashMap;
use screeps::Part::{Carry, Move, Work};
use screeps::{game, Direction, ObjectId, Part, RoomName, SpawnOptions, StructureSpawn, CREEP_SPAWN_TIME};
use std::cell::RefCell;
use std::cmp::max;
use std::collections::BTreeMap;
use std::collections::Bound::Included;
use crate::creeps::fresh_creep_name;
use crate::kernel::condition::Condition;

thread_local! {
    static SPAWN_SCHEDULES: RefCell<FxHashMap<RoomName, RoomSpawnSchedule>> = RefCell::new(FxHashMap::default());
}

fn with_spawn_schedule<F, R>(room_name: RoomName, mut f: F) -> R
where
    F: FnMut(&mut RoomSpawnSchedule) -> R,
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
    scheduled_spawns: FxHashMap<ObjectId<StructureSpawn>, BTreeMap<u32, SpawnRequest>>,
    // /// Map of spawn schedules for each creep role.
    // schedules: FxHashMap<CreepRole, SpawnSchedule>,
}

/// A scheduled spawn intent to spawn a specific creep at specific place.
struct SpawnRequest {
    role: CreepRole,
    body: CreepBody,
    priority: u8,
    // preferred_spawn: Vec<PreferredSpawn>,
    preferred_tick: (u32, u32),
    // limit_tick: (u32, u32),
}

#[derive(Debug, Clone)]
struct CreepBody {
    parts: Vec<Part>,
}

impl CreepBody {
    fn spawn_duration(&self) -> u32 {
        self.parts.len() as u32 * CREEP_SPAWN_TIME
    }
}

struct PreferredSpawn {
    spawn: ObjectId<StructureSpawn>,
    directions: Vec<Direction>,
    /// Extra energy cost incurred by selecting this spawn.
    extra_cost: u32,
}

/// Issue a one-shot order to spawn a creep.
fn spawn_creep(room_name: RoomName, request: SpawnRequest) {
    with_spawn_schedule(room_name, |room_spawn_schedule| {
        let mut selected_spawn_data = None;

        for (&spawn_id, spawn_schedule) in room_spawn_schedule.scheduled_spawns.iter() {
            if let Some(spawn_tick) = find_spawn_tick(&request, spawn_schedule) {
                selected_spawn_data = Some((spawn_id, spawn_tick));
                break;
            }
        }

        selected_spawn_data
    });

    // TODO schedule the spawn
}

fn find_spawn_tick(request: &SpawnRequest, schedule: &BTreeMap<u32, SpawnRequest>) -> Option<u32> {
    let spawn_duration = request.body.spawn_duration();
    let min_spawn_tick = max(game_tick(), request.preferred_tick.0 - spawn_duration);
    let max_spawn_tick = request.preferred_tick.1 - spawn_duration;

    // Trying min_spawn_tick and each tick after a creep finishes spawning.
    let mut spawn_tick = min_spawn_tick;
    let mut cursor = schedule.lower_bound(Included(&request.preferred_tick.0));
    loop {
        if let Some((other_spawn_tick, other_spawn_request)) = cursor.key_value() {
            if spawn_tick + spawn_duration < *other_spawn_tick {
                return Some(spawn_tick);
            } else {
                spawn_tick = *other_spawn_tick + other_spawn_request.body.spawn_duration() + 1;
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

pub fn creep_body(role: CreepRole, resources: &RoomResources) -> Vec<Part> {
    match role {
        CreepRole::Craftsman => {
            if resources.spawn_energy >= 600 {
                vec![Work, Work, Work, Work, Move, Move, Carry, Carry]
            } else if resources.spawn_energy >= 300 {
                vec![Work, Work, Move, Carry]
            } else {
                vec![Work, Move, Carry]
            }
        }
        CreepRole::Scout => {
            vec![Move]
        }
    }
}

/// Issue the intents to spawn creeps in given tick.
pub fn spawn_room_creeps(room_name: RoomName) {
    // let resources = room_resources(room_name);

    with_spawn_schedule(room_name, |room_spawn_schedule| {
        for (spawn_id, spawn_schedule) in room_spawn_schedule.scheduled_spawns.iter_mut() {
            if let Some((&spawn_tick, request)) = spawn_schedule.first_key_value() {
                if spawn_tick <= game_tick() {
                    debug!("Attempting to spawn {:?} in {}.", request.role, room_name);

                    if let Some(spawn) = game::get_object_by_id_typed(spawn_id) {
                        let name = fresh_creep_name(request.role);
                        let spawn_options = SpawnOptions::default();
                        if spawn
                            .spawn_creep_with_options(&request.body.parts, &name, &spawn_options)
                            .to_bool_and_warn(&format!(
                                "Failed to spawn a creep in {} at spawn {}",
                                room_name, spawn_id
                            ))
                        {
                            // TODO Inform waiting processes after the creep spawns.
                            // TODO To do this, it'd be useful to introduce waiting for resource where manager asks
                            //      kernel for a unique handler to Rc<RefCell<T>> and when something takes place, it
                            //      sets T and wakes up that process if it was awaiting. This is needed to wait for
                            //      a spawned creep, to get X resources in storage/extensions, to coordinate spawning of
                            //      a quad and more. It'd also be nice to have ability to cancel such a request.
                            //      Currently, waiting for a creep can be just sleep(), waiting for resources requires
                            //      active checking anyway, cancelling can be made by direct call. select/join is
                            //      needed, though.
                            //      Or not, it can be rescheduled, potentially even earlier.
                            // TODO for now make a Future to wait for creep spawn
                            let spawn_condition = Condition::<Creep>::new();
                            // TODO save it somewhere and signal on successful spawn
                        } else {
                            // TODO Inform waiting processes that the spawning failed.
                        }
                    } else {
                        warn!("Failed to find spawn {} in {}.", spawn_id, room_name);
                        // TODO Inform waiting processes that the spawning failed.
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
