use crate::creep::CreepRole;
use crate::creeps::CreepRef;
use crate::game_time::game_tick;
use crate::geometry::room_xy::RoomXYUtils;
use crate::hauling::MaybeSpawned::{Spawned, Spawning};
use crate::kernel::condition::Condition;
use crate::kernel::process::Priority;
use crate::priorities::HAULER_SPAWN_PRIORITY;
use crate::resources::room_resources;
use crate::room_state::room_states::with_room_state;
use crate::spawning::{schedule_creep, CreepBody, PreferredSpawn, SpawnRequest};
use crate::u;
use rustc_hash::FxHashMap;
use screeps::Part::{Carry, Move};
use screeps::StructureType::Storage;
use screeps::{Position, RoomName};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::iter::repeat;

thread_local! {
    static HAUL_SCHEDULES: RefCell<FxHashMap<RoomName, RoomHaulSchedule>> = RefCell::new(FxHashMap::default());
}

fn with_hauling_schedule<F, R>(room_name: RoomName, f: F) -> R
where
    F: FnOnce(&mut RoomHaulSchedule) -> R,
{
    // TODO need scan data to create the schedule
    HAUL_SCHEDULES.with(|states| {
        let mut borrowed_states = states.borrow_mut();
        let room_spawn_schedule = borrowed_states
            .entry(room_name)
            .or_insert_with(RoomHaulSchedule::default);
        f(room_spawn_schedule)
    })
}

#[derive(Default)]
struct RoomHaulSchedule {
    /// Vector of respective haulers' data.
    hauler_data: Vec<HaulerData>,
}

struct HaulerData {
    /// Hauler creep. Either alive or scheduled to be spawned.
    hauler: MaybeSpawned,
    /// Map from ticks to haul events beginning on these ticks.
    schedule: BTreeMap<u32, HaulEvent>,
    /// Data on what the hauler is hauling now.
    current: Option<HaulEvent>,
}

enum MaybeSpawned {
    Spawned(CreepRef),
    Spawning(Condition<Option<CreepRef>>),
}

impl MaybeSpawned {
    fn as_ref(&self) -> Option<&CreepRef> {
        match self {
            Spawned(creep_ref) => Some(creep_ref),
            Spawning(_) => None,
        }
    }
}

/// The hauler ID is the offset int the `current_haulers` vector.
type HaulerId = usize;

/// A scheduled spawn.
struct HaulEvent {
    request: HaulRequest,
}

pub struct HaulRequest {
    // TODO
    pub source: Position,
    // TODO
    pub target: Position,
    pub amount: u32,
    pub request_tick: u32,
    pub amount_per_tick: u32,
    pub max_amount: u32,
    pub priority: Priority,
    pub preferred_tick: (u32, u32),
}

pub fn schedule_haul(room_name: RoomName, request: HaulRequest) {}

// TODO also cross-room version
/// Execute hauling of resources within a room.
pub fn haul_resources(room_name: RoomName) {
    with_hauling_schedule(room_name, |schedule| {
        while schedule.hauler_data.len() < 3 {
            let spawn_request = hauler_spawn_request(room_name);
            if let Some(scheduled_creep) = schedule_creep(room_name, spawn_request) {
                schedule.hauler_data.push(HaulerData {
                    hauler: MaybeSpawned::Spawning(scheduled_creep),
                    schedule: Default::default(),
                    current: None,
                });
            }
        }

        for hauler_data in schedule.hauler_data.iter_mut() {
            let maybe_creep_ref = match &hauler_data.hauler {
                Spawned(creep_ref) => {
                    if creep_ref.borrow().exists() {
                        Some(creep_ref)
                    } else {
                        None
                    }
                }
                Spawning(spawn_condition) => {
                    match spawn_condition.check() {
                        None => {
                            // Waiting for the hauler to spawn.
                            continue;
                        }
                        Some(None) => {
                            // Failed to spawn the hauler. Trying again.
                            None
                        }
                        Some(Some(creep_ref)) => {
                            // Spawned the hauler.
                            hauler_data.hauler = Spawned(creep_ref);
                            hauler_data.hauler.as_ref()
                        }
                    }
                }
            };

            if hauler_data.current.is_none() {
                if let Some(event) = hauler_data.schedule.first_entry() {
                    if *event.key() >= game_tick() {
                        // Assigning the haul event to the hauler.
                        hauler_data.current = Some(event.remove());
                    }
                }
            }

            if let Some(event) = hauler_data.current.as_ref() {}
        }
    });
}

fn hauler_spawn_request(room_name: RoomName) -> SpawnRequest {
    // Prefer being spawned closer to the storage.
    let preferred_spawns = u!(with_room_state(room_name, |room_state| {
        let mut spawns = room_state
            .spawns
            .iter()
            .map(|spawn_data| {
                (
                    spawn_data.xy,
                    PreferredSpawn {
                        id: spawn_data.id,
                        directions: Vec::new(),
                        extra_cost: 0,
                    },
                )
            })
            .collect::<Vec<_>>();
        if let Some(storage_xy) = room_state
            .structures
            .get(&Storage)
            .and_then(|xys| xys.iter().next().cloned())
        {
            spawns.sort_by_key(|(spawn_xy, _)| spawn_xy.dist(storage_xy));
        }
        spawns
            .into_iter()
            .map(|(_, preferred_spawn)| preferred_spawn)
            .collect::<Vec<_>>()
    }));

    let min_preferred_tick = game_tick();
    let max_preferred_tick = game_tick() + 1000;

    SpawnRequest {
        role: CreepRole::Hauler,
        body: hauler_body(room_name),
        priority: HAULER_SPAWN_PRIORITY,
        preferred_spawns,
        preferred_tick: (min_preferred_tick, max_preferred_tick),
    }
}

fn hauler_body(room_name: RoomName) -> CreepBody {
    let resources = room_resources(room_name);

    let parts = if resources.spawn_energy >= 550 {
        repeat([Carry, Move]).take(5).flatten().collect::<Vec<_>>()
    } else {
        vec![Carry, Move, Carry, Move, Carry, Move]
    };

    CreepBody::new(parts)
}
