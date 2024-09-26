use crate::creeps::creep::{CreepBody, CreepRole};
use crate::game_time::game_tick;
use crate::geometry::room_xy::RoomXYUtils;
use crate::kernel::process::Priority;
use crate::priorities::HAULER_SPAWN_PRIORITY;
use crate::resources::room_resources;
use crate::room_state::room_states::with_room_state;
use crate::spawning::{PreferredSpawn, SpawnRequest};
use crate::u;
use rustc_hash::FxHashMap;
use screeps::Part::{Carry, Move};
use screeps::StructureType::Storage;
use screeps::{ObjectId, Position, RawObjectId, Resource, ResourceType, ReturnCode, RoomName, Transferable, Withdrawable};
use std::cell::RefCell;
use std::iter::repeat;
use log::debug;
use screeps::game::get_object_by_id_erased;
use wasm_bindgen::JsCast;
use crate::creep_error::CreepError;
use crate::kernel::sleep::sleep;
use crate::spawn_pool::{SpawnPool, SpawnPoolOptions};
use crate::travel::{travel, TravelSpec};

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

pub type RequestId = u32;

#[derive(Default)]
struct RoomHaulSchedule {
    withdraw_requests: FxHashMap<RequestId, RawWithdrawRequest>,
    store_requests: FxHashMap<RequestId, RawStoreRequest>,
    next_id: u32,
}

/// The hauler ID is the offset int the `current_haulers` vector.
type HaulerId = usize;

#[derive(Debug)]
pub struct WithdrawRequest<T> {
    /// Room name of the room responsible for providing the hauler.
    pub room_name: RoomName,
    pub target: ObjectId<T>,
    pub xy: Option<Position>,
    pub amount: u32,
    // pub amount_per_tick: u32,
    // pub max_amount: u32,
    pub priority: Priority,
    // pub preferred_tick: (u32, u32),
}

#[derive(Debug)]
struct RawWithdrawRequest {
    target: RawObjectId,
    pickupable: bool,
    xy: Option<Position>,
    amount: u32,
    // amount_per_tick: u32,
    // max_amount: u32,
    priority: Priority,
    // preferred_tick: (u32, u32),
    // request_tick: u32,
}

#[derive(Debug)]
pub struct StoreRequest<T>
where
    T: Transferable,
{
    pub room_name: RoomName,
    pub target: ObjectId<T>,
    pub xy: Option<Position>,
    pub amount: u32,
    pub priority: Priority,
    // pub preferred_tick: (u32, u32),
}

#[derive(Debug)]
pub struct RawStoreRequest {
    pub target: RawObjectId,
    pub xy: Option<Position>,
    pub amount: u32,
    pub priority: Priority,
    // pub preferred_tick: (u32, u32),
}

// TODO Option to not drop it.
pub struct WithdrawRequestId {
    room_name: RoomName,
    id: RequestId,
    droppable: bool,
}

impl Drop for WithdrawRequestId {
    fn drop(&mut self) {
        with_hauling_schedule(self.room_name, |schedule| {
            // TODO Cancelling haul that is already in progress.
            schedule.withdraw_requests.remove(&self.id);
        });
    }
}

pub struct StoreRequestId {
    room_name: RoomName,
    id: RequestId,
    droppable: bool,
}

impl Drop for StoreRequestId {
    fn drop(&mut self) {
        with_hauling_schedule(self.room_name, |schedule| {
            // TODO Cancelling haul that is already in progress.
            if self.droppable {
                debug!("Dropping store request {}/{}.", self.room_name, self.id);
                schedule.store_requests.remove(&self.id);
            }
        });
    }
}

pub fn schedule_withdraw<T>(withdraw_request: &WithdrawRequest<T>, replaced_request_id: Option<WithdrawRequestId>) -> WithdrawRequestId
    where
        T: Withdrawable,
{
    let raw_withdraw_request = RawWithdrawRequest {
        target: withdraw_request.target.into(),
        pickupable: false,
        xy: withdraw_request.xy,
        amount: withdraw_request.amount,
        // amount_per_tick: withdraw_request.amount_per_tick,
        // max_amount: withdraw_request.max_amount,
        priority: withdraw_request.priority,
        // preferred_tick: withdraw_request.preferred_tick,
        // request_tick: game_tick(),
    };
    schedule_raw_withdraw_request(withdraw_request.room_name, raw_withdraw_request, replaced_request_id)
}

pub fn schedule_pickup(withdraw_request: WithdrawRequest<Resource>, replaced_request_id: Option<WithdrawRequestId>) -> WithdrawRequestId {
    let raw_withdraw_request = RawWithdrawRequest {
        target: withdraw_request.target.into(),
        pickupable: true,
        xy: withdraw_request.xy,
        amount: withdraw_request.amount,
        // amount_per_tick: withdraw_request.amount_per_tick,
        // max_amount: withdraw_request.max_amount,
        priority: withdraw_request.priority,
        // preferred_tick: withdraw_request.preferred_tick,
        // request_tick: game_tick(),
    };
    schedule_raw_withdraw_request(withdraw_request.room_name, raw_withdraw_request, replaced_request_id)
}

fn schedule_raw_withdraw_request(room_name: RoomName, request: RawWithdrawRequest, mut replaced_request_id: Option<WithdrawRequestId>) -> WithdrawRequestId {
    if let Some(mut existing_replaced_request_id) = replaced_request_id.take() {
        existing_replaced_request_id.droppable = false;
    }

    with_hauling_schedule(room_name, |schedule| {
        let id = schedule.next_id;
        schedule.next_id += 1;
        schedule.withdraw_requests.insert(id, request);
        WithdrawRequestId {
            room_name,
            id,
            droppable: true,
        }
    })
}

pub fn schedule_store<T>(store_request: StoreRequest<T>, mut replaced_request_id: Option<StoreRequestId>) -> StoreRequestId
where
    T: Transferable,
{
    // TODO Maybe just assume it's being dropped outside?
    if let Some(mut existing_replaced_request_id) = replaced_request_id.take() {
        existing_replaced_request_id.droppable = false;
    }

    let raw_store_request = RawStoreRequest {
        target: store_request.target.into(),
        xy: store_request.xy,
        amount: store_request.amount,
        priority: store_request.priority,
        // preferred_tick: store_request.preferred_tick,
    };
    with_hauling_schedule(store_request.room_name, |schedule| {
        let id = schedule.next_id;
        schedule.next_id += 1;
        schedule.store_requests.insert(id, raw_store_request);
        StoreRequestId {
            room_name: store_request.room_name,
            id,
            droppable: true,
        }
    })
}

/// Execute hauling of resources of haulers assigned to given room.
/// Withdraw and store requests are registered in the system and the system assigns them to fre
/// haulers. One or more withdraw event is paired with one or more store events. There are special
/// withdraw and store events for the storage which may not be paired with one another.
pub async fn haul_resources(room_name: RoomName) {
    let base_spawn_request = u!(with_room_state(room_name, |room_state| {
        let body = hauler_body(room_name);

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

        SpawnRequest {
            role: CreepRole::Hauler,
            body,
            priority: HAULER_SPAWN_PRIORITY,
            preferred_spawns,
            preferred_tick: (0, 0),
        }
    }));
    
    let mut spawn_pool = SpawnPool::new(room_name, base_spawn_request, SpawnPoolOptions::default());
    
    loop {
        spawn_pool.with_spawned_creep(|creep_ref| async move {
            loop {
                let mut maybe_withdraw_request = None;
                let mut maybe_store_request = None;

                with_hauling_schedule(room_name, |schedule| {
                    debug!("{} searching for withdraw/pickup and store requests.", creep_ref.borrow().name);
                    debug!("{:?}", schedule.withdraw_requests);
                    debug!("{:?}", schedule.store_requests);

                    if schedule.withdraw_requests.is_empty() || schedule.store_requests.is_empty() {
                        return;
                    }

                    let creep_pos = creep_ref.borrow().pos();

                    let maybe_closest_withdraw_request_data = schedule
                        .withdraw_requests
                        .iter()
                        .filter_map(|(&id, request)| {
                            request.xy.map(|xy| (id, xy, xy.get_range_to(creep_pos)))
                        })
                        .min_by_key(|&(_, _, d)| d);

                    if let Some((closest_withdraw_request_id, withdraw_xy, _)) = maybe_closest_withdraw_request_data {
                        let maybe_closest_store_request_data = schedule
                            .store_requests
                            .iter()
                            .filter_map(|(&id, request)| {
                                request.xy.map(|xy| (id, xy.get_range_to(withdraw_xy)))
                            })
                            .min_by_key(|&(_, d)| d);

                        if let Some((closest_store_request_id, _)) = maybe_closest_store_request_data {
                            maybe_withdraw_request = schedule.withdraw_requests.remove(&closest_withdraw_request_id);
                            maybe_store_request = schedule.store_requests.remove(&closest_store_request_id);
                        }
                    }
                });

                if let Some(withdraw_request) = maybe_withdraw_request.take() {
                    let store_request = u!(maybe_store_request.take());

                    let result: Result<(), CreepError> = (async {
                        let withdraw_travel_spec = TravelSpec {
                            target: u!(withdraw_request.xy),
                            range: 1,
                        };

                        let res = travel(&creep_ref, withdraw_travel_spec).await?;

                        if withdraw_request.pickupable {
                            if let Some(raw_resource) = get_object_by_id_erased(&withdraw_request.target) {
                                let resource = raw_resource.unchecked_into::<Resource>();
                                if creep_ref.borrow().pickup(&resource) != ReturnCode::Ok {
                                    return Err(CreepError::CreepPickupFailed);
                                }
                            } else {
                                return Err(CreepError::CreepPickupFailed);
                            }
                        } else {
                            // TODO
                        }

                        let store_travel_spec = TravelSpec {
                            target: u!(store_request.xy),
                            range: 1,
                        };

                        travel(&creep_ref, store_travel_spec).await?;
                        // TODO Minimum 1 tick of pause after withdraw even if in correct place.

                        if let Some(store_target) = get_object_by_id_erased(&store_request.target) {
                            creep_ref.borrow().unchecked_transfer(&store_target, ResourceType::Energy, None);
                        } else {
                            return Err(CreepError::CreepTransferFailed);
                        }

                        Ok(())
                    }).await;

                    if let Err(e) = result {
                        debug!("Error when hauling: {:?}.", e);
                        sleep(1).await;
                    }
                } else {
                    sleep(1).await;
                }
            }
        });
        
        sleep(1).await;
    }
}

/*
pub fn old_haul_resources(room_name: RoomName) {
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
 */

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
