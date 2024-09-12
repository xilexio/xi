use crate::creep::{CreepBody, CreepRole};
use crate::creeps::CreepRef;
use crate::game_time::game_tick;
use crate::geometry::room_xy::RoomXYUtils;
use crate::hauling::MaybeSpawned::{Spawned, Spawning};
use crate::kernel::condition::Condition;
use crate::kernel::process::Priority;
use crate::priorities::HAULER_SPAWN_PRIORITY;
use crate::resources::room_resources;
use crate::room_state::room_states::with_room_state;
use crate::spawning::{schedule_creep, PreferredSpawn, SpawnRequest};
use crate::u;
use rustc_hash::FxHashMap;
use screeps::Part::{Carry, Move};
use screeps::StructureType::Storage;
use screeps::{ObjectId, Position, RawObjectId, Resource, RoomName, Transferable, Withdrawable};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::iter::repeat;
use crate::utils::map_utils::MultiMapUtils;

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
    /// Vector of respective haulers' creep data, schedules and current tasks.
    hauler_data: Vec<HaulerData>,
    withdraw_requests: BTreeMap<u32, Vec<RawWithdrawRequest>>,
    store_requests: BTreeMap<u32, Vec<RawStoreRequest>>,
    /// Map from ticks to all unassigned hauling tasks.
    events: BTreeMap<u32, HaulEvent>,
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

pub struct WithdrawRequest<T> {
    /// Room name of the room responsible for providing the hauler.
    pub room_name: RoomName,
    pub target: ObjectId<T>,
    pub xy: Option<Position>,
    pub amount: u32,
    pub amount_per_tick: u32,
    pub max_amount: u32,
    pub priority: Priority,
    pub preferred_tick: (u32, u32),
}

struct RawWithdrawRequest {
    target: RawObjectId,
    pickupable: bool,
    xy: Option<Position>,
    amount: u32,
    amount_per_tick: u32,
    max_amount: u32,
    priority: Priority,
    preferred_tick: (u32, u32),
    request_tick: u32,
}

pub struct StoreRequest<T>
where
    T: Transferable,
{
    pub room_name: RoomName,
    pub target: ObjectId<T>,
    pub xy: Option<Position>,
    pub amount: u32,
    pub priority: Priority,
    pub preferred_tick: (u32, u32),
}

pub struct RawStoreRequest {
    pub target: RawObjectId,
    pub xy: Option<Position>,
    pub amount: u32,
    pub priority: Priority,
    pub preferred_tick: (u32, u32),
}

pub fn schedule_withdraw<T>(withdraw_request: WithdrawRequest<T>)
    where
        T: Withdrawable,
{
    let raw_withdraw_request = RawWithdrawRequest {
        target: withdraw_request.target.into(),
        pickupable: false,
        xy: withdraw_request.xy,
        amount: withdraw_request.amount,
        amount_per_tick: withdraw_request.amount_per_tick,
        max_amount: withdraw_request.max_amount,
        priority: withdraw_request.priority,
        preferred_tick: withdraw_request.preferred_tick,
        request_tick: game_tick(),
    };
    with_hauling_schedule(withdraw_request.room_name, |schedule| {
        schedule.withdraw_requests.push_or_insert(withdraw_request.preferred_tick.0, raw_withdraw_request);
    });
}

pub fn schedule_pickup(withdraw_request: WithdrawRequest<Resource>) {
    let raw_withdraw_request = RawWithdrawRequest {
        target: withdraw_request.target.into(),
        pickupable: true,
        xy: withdraw_request.xy,
        amount: withdraw_request.amount,
        amount_per_tick: withdraw_request.amount_per_tick,
        max_amount: withdraw_request.max_amount,
        priority: withdraw_request.priority,
        preferred_tick: withdraw_request.preferred_tick,
        request_tick: game_tick(),
    };
    with_hauling_schedule(withdraw_request.room_name, |schedule| {
        schedule.withdraw_requests.push_or_insert(withdraw_request.preferred_tick.0, raw_withdraw_request);
    });
}

pub fn schedule_store<T>(store_request: StoreRequest<T>)
where
    T: Transferable,
{
    let raw_store_request = RawStoreRequest {
        target: store_request.target.into(),
        xy: store_request.xy,
        amount: store_request.amount,
        priority: store_request.priority,
        preferred_tick: store_request.preferred_tick,
    };
    with_hauling_schedule(store_request.room_name, |schedule| {
        schedule.store_requests.push_or_insert(store_request.preferred_tick.0, raw_store_request);
    });
}

/// Execute hauling of resources of haulers assigned to given room.
/// Withdraw and store requests are registered in the system and the system assigns them to free haulers.
/// One or more withdraw event is paired with one or more store events. There are special withdraw and store events
/// for the storage which may not be paired with one another.
// TODO
pub fn haul_resources(room_name: RoomName) {
    with_hauling_schedule(room_name, |schedule| {
        while schedule.hauler_data.len() < 3 {
            let spawn_request = hauler_spawn_request(room_name);
            // If the spawning immediately fails, it will be retried in subsequent ticks.
            if let Some(scheduled_hauler) = schedule_creep(room_name, spawn_request) {
                // schedule.hauler_data.push(HaulerData {
                //     hauler: Spawning(scheduled_hauler),
                //     schedule: Default::default(),
                //     current: None,
                // });
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

            if let Some(creep_ref) = maybe_creep_ref {
                // Hauling.
                if hauler_data.current.is_none() {
                    // TODO Maybe have one common queue plus queues for each hauler, but only lasting til the end of its lifetime?
                    if let Some(event) = hauler_data.schedule.first_entry() {
                        // TODO check for a good time to take it.
                        if *event.key() >= game_tick() {
                            // Assigning the haul event to the hauler.
                            hauler_data.current = Some(event.remove());
                        }
                    }
                }

                if let Some(event) = hauler_data.current.as_ref() {
                    // Spawning a separate process for the event.
                    // TODO move to source, withdraw source, move to target, store in target
                }
            } else {
                // Spawning a hauler.
                let spawn_request = hauler_spawn_request(room_name);
                // If the spawning immediately fails, it will be retried in subsequent ticks.
                if let Some(scheduled_hauler) = schedule_creep(room_name, spawn_request) {
                    // hauler_data.hauler = Spawning(scheduled_hauler);
                }
            }
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
