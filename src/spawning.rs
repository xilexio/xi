use crate::creeps::creep::{CreepBody, CreepRole};
use crate::creeps::{register_creep, CreepRef};
use crate::game_tick::game_tick;
use crate::kernel::schedule;
use crate::kernel::sleep::sleep;
use crate::priorities::CREEP_REGISTRATION_PRIORITY;
use crate::room_state::room_states::with_room_state;
use crate::u;
use log::{debug, trace, warn};
use rustc_hash::{FxHashMap, FxHashSet};
use screeps::{
    game,
    Direction,
    ObjectId,
    RoomName,
    SpawnOptions,
    StructureSpawn,
};
use std::cell::RefCell;
use std::collections::{BTreeMap, Bound};
use std::rc::Rc;
use crate::errors::XiError;
use crate::errors::XiError::SpawnRequestTickInThePast;
use crate::utils::priority::Priority;
use crate::utils::uid::UId;

thread_local! {
    static SPAWN_SCHEDULES: RefCell<FxHashMap<RoomName, RoomSpawnSchedule>> = RefCell::new(FxHashMap::default());
}

fn with_spawn_schedule<F, R>(room_name: RoomName, f: F) -> R
where
    F: FnOnce(&mut RoomSpawnSchedule) -> R,
{
    // TODO A way to react to changes in structures in a room by cancelling or modifying promises.
    // TODO Helper functions in another module to find closest spawn and direction.
    SPAWN_SCHEDULES.with(|states| {
        let mut borrowed_states = states.borrow_mut();
        let room_spawn_schedule = borrowed_states
            .entry(room_name)
            .or_default();
        f(room_spawn_schedule)
    })
}

#[derive(Default)]
struct RoomSpawnSchedule {
    /// Future spawns ordered by preferred tick.
    future_spawns: BTreeMap<u32, FxHashMap<UId, SpawnEvent>>,
    /// Current spawns ordered by priority. Usually empty unless there are insufficient resources
    /// to spawn a creep.
    current_spawns: BTreeMap<(Priority, UId), SpawnEvent>,
    /// Spawn events for creeps currently being spawned.
    spawns_in_progress: FxHashMap<ObjectId<StructureSpawn>, Option<SpawnEvent>>,
}

/// A scheduled spawn.
struct SpawnEvent {
    request: SpawnRequest,
    promise: SpawnPromiseRef,
    energy_cost: u32,
    spawn_duration: u32,
    end_tick: Option<u32>,
}

/// A promise to spawn a creep. It can be used to check the progress, whether the spawning was
/// cancelled or to get the creep after it was spawned.
pub struct SpawnPromise {
    pub id: UId,
    pub spawn_id: Option<ObjectId<StructureSpawn>>,
    pub spawn_end_tick: Option<u32>,
    pub cancelled: bool,
    pub creep: Option<CreepRef>,
}

impl SpawnPromise {
    fn new() -> Self {
        SpawnPromise {
            id: UId::new(),
            spawn_id: None,
            spawn_end_tick: None,
            cancelled: false,
            creep: None,
        }
    }
}

pub type SpawnPromiseRef = Rc<RefCell<SpawnPromise>>;

/// A request with information needed to spawn the creep.
#[derive(Debug, Clone)]
pub struct SpawnRequest {
    pub role: CreepRole,
    pub body: CreepBody,
    pub priority: Priority,
    /// Spawns in the order of preference. Must list all valid spawns and be ordered by `extra_cost`.
    pub preferred_spawns: Vec<PreferredSpawn>,
    pub tick: (u32, u32),
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
pub fn schedule_creep(room_name: RoomName, request: SpawnRequest) -> Result<SpawnPromiseRef, XiError> {
    with_spawn_schedule(room_name, move |room_spawn_schedule| {
        let spawn_promise = SpawnPromise::new();
        let id = spawn_promise.id;

        let current_tick = game_tick();
        let preferred_spawn_start_tick = request.tick.0;

        if preferred_spawn_start_tick < current_tick {
            return Err(SpawnRequestTickInThePast);
        }

        // Create a SpawnEvent and add it to the schedule.
        let spawn_promise_ref = Rc::new(RefCell::new(spawn_promise));
        let energy_cost = request.body.energy_cost();
        let spawn_duration = request.body.spawn_duration();
            
        let spawn_event = SpawnEvent {
            request,
            promise: spawn_promise_ref.clone(),
            energy_cost,
            spawn_duration,
            end_tick: None,
        };

        room_spawn_schedule
            .future_spawns
            .entry(preferred_spawn_start_tick)
            .or_default()
            .insert(id, spawn_event);

        Ok(spawn_promise_ref)
    })
}

/// Cancels scheduled spawn event.
/// Does not cancel creeps that are already spawning. This function is rather inefficient.
pub fn cancel_scheduled_creep(room_name: RoomName, spawn_promise: SpawnPromiseRef) {
    let mut spawn_promise = spawn_promise.borrow_mut();

    spawn_promise.cancelled = true;

    with_spawn_schedule(room_name, |room_spawn_schedule| {
        // If the spawn_id is set then the event can only be in `spawns_in_progress`.
        // Skipping this case and just letting the creep spawn to be used as an idle creep later.
        if spawn_promise.spawn_id.is_none() {
            // Removing the spawn event from `current_spawns`.
            let maybe_removed_event = room_spawn_schedule.current_spawns.extract_if(|&(_, id), _| id == spawn_promise.id).next();
            // If it wasn't there, the only remaining place is `future_spawns`.
            if maybe_removed_event.is_none() {
                let mut cursor = room_spawn_schedule.future_spawns.lower_bound_mut(Bound::Unbounded);
                while let Some((_, events)) = cursor.next() {
                    if events.contains_key(&spawn_promise.id) {
                        events.remove(&spawn_promise.id);
                        if events.is_empty() {
                            cursor.remove_prev();
                        }
                        break;
                    }
                }
            }
        }
    })
}

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
        let creep = register_creep(event.request.role);

        // Issuing the spawn intent.
        let spawn_options = SpawnOptions::default();
        let spawn_result = spawn
            .spawn_creep_with_options(&event.request.body.parts, &creep.borrow().name, &spawn_options);

        if spawn_result.is_err() {
            warn!(
                "Failed to spawn {} in spawn {} in {}.",
                event.request.role, spawn_id, room_name
            );
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
                        "Failed to spawn {} in {} due to lost spawn.",
                        event.request.role, room_name
                    );
                    event.promise.borrow_mut().cancelled = true;
                }
            }
        });
    });
}