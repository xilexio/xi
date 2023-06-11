use crate::creep::{Creep, CreepRole};
use rustc_hash::FxHashMap;
use screeps::{game, Direction, ObjectId, RoomName, SpawnOptions, StructureSpawn, Part};
use std::cell::RefCell;
use std::collections::BTreeMap;
use screeps::Part::{Carry, Move, Work};
use crate::resources::{room_resources, RoomResources};
use crate::utils::map_utils::OrderedMultiMapUtils;

#[derive(Default)]
struct SpawnManager {
    rooms: FxHashMap<RoomName, RoomSpawnManager>,
}

thread_local! {
    static SPAWN_MANAGER: RefCell<FxHashMap<RoomName, SpawnManager>> = RefCell::new(FxHashMap::default());
}

fn with_mut_spawn_manager<F, R>(room_name: RoomName, mut f: F) -> Option<R>
where
    F: FnMut(&mut RoomSpawnManager) -> R,
{
    SPAWN_MANAGER.with(|states| {
        states.borrow_mut().get_mut(&room_name).map(|spawn_manager| {
            f(spawn_manager
                .rooms
                .entry(room_name)
                .or_insert(RoomSpawnManager::default()))
        })
    })
}

#[derive(Default)]
struct RoomSpawnManager {
    /// Map from ticks to spawn events at that tick.
    scheduled_spawns: BTreeMap<u32, Vec<SpawnEvent>>,
    /// Map of spawn schedules for each creep role.
    schedules: FxHashMap<CreepRole, SpawnSchedule>,
}

/// A scheduled spawn intent to spawn a specific creep at specific place.
struct SpawnEvent {
    spawn: ObjectId<StructureSpawn>,
    role: CreepRole,
    priority: u8,
    preferred_directions: Option<Vec<Direction>>,
    delegate: Box<dyn FnMut(Creep)>,
}

/// A schedule with information how many creeps of given type should be present in a room with priorities.
struct SpawnSchedule {
    room_name: RoomName,
    role: CreepRole,
    /// N-th element is the priority of spawning n-th creep. The length of the vector is how many creeps are supposed
    /// to be spawned when the resources are plentiful.
    priorities: Vec<u8>,
    preferred_spawn: Option<ObjectId<StructureSpawn>>,
    preferred_directions: Option<Vec<Direction>>,
    delegate: Box<dyn FnMut(Creep)>,
}

/// Update the schedule to spawn creeps. Scheduling a creep to be spawned at given intervals and capacity.
pub fn update_creep_spawn_schedule() {}

/// Issue a one-shot order to spawn a creep.
pub fn spawn_creep() {}

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
    let resources = room_resources(room_name);

    with_mut_spawn_manager(room_name, |room_spawn_manager| {
        while let Some((&event_tick, _)) = room_spawn_manager.scheduled_spawns.first_key_value() {
            if event_tick <= game::time() {
                if let Some((_, spawn_event)) = room_spawn_manager.scheduled_spawns.pop_from_first() {
                    if let Some(spawn) = game::get_object_by_id_typed(&spawn_event.spawn) {
                        // TODO Decide on the body based on the role, energy available now, energy capacity and other factors.
                        //      If the energy is not available now, it should be rescheduled, potentially moving other, lower priority spawns for later.
                        //      Make some kind of statistics about energy levels and if there is not enough energy in the room or it exceeds maximum energy capacity,
                        //      try to make a more energy efficient/less costly version.
                        //      If it was rescheduled for other than energy reasons for some time, optionally bump the priority.
                        //      Introduce waiting for energy into the schedule. Move all ticks forward.
                        let body = creep_body(spawn_event.role, &resources);
                        let name = "xxx"; // TODO creep role and a counter with rotation
                                          // TODO energy_structures from fast filler to nearest extensions to furthest extensions
                                          // TODO directions
                        spawn.spawn_creep_with_options(&body, name, &SpawnOptions::default());
                    } else {
                        // The spawn does not exist. It might have been destroyed.
                        // TODO
                    }
                }
            }
        }
    });
}
