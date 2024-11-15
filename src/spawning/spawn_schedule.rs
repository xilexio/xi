use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use rustc_hash::FxHashMap;
use screeps::{Direction, ObjectId, RoomName, StructureSpawn};
use crate::creeps::creep::{CreepBody, CreepRole};
use crate::creeps::CreepRef;
use crate::utils::priority::Priority;
use crate::utils::uid::UId;

thread_local! {
    static SPAWN_SCHEDULES: RefCell<FxHashMap<RoomName, RoomSpawnSchedule>> = RefCell::new(FxHashMap::default());
}

pub(crate) fn with_spawn_schedule<F, R>(room_name: RoomName, f: F) -> R
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
pub(crate) struct RoomSpawnSchedule {
    /// Future spawns ordered by preferred tick.
    pub future_spawns: BTreeMap<u32, FxHashMap<UId, SpawnEvent>>,
    /// Current spawns ordered by priority. Usually empty unless there are insufficient resources
    /// to spawn a creep.
    pub current_spawns: BTreeMap<(Priority, UId), SpawnEvent>,
    /// Spawn events for creeps currently being spawned.
    pub spawns_in_progress: FxHashMap<ObjectId<StructureSpawn>, Option<SpawnEvent>>,
}

/// A scheduled spawn.
pub(crate) struct SpawnEvent {
    pub request: SpawnRequest,
    pub promise: SpawnPromiseRef,
    pub energy_cost: u32,
    pub spawn_duration: u32,
}

/// A promise to spawn a creep. It can be used to check the progress, whether the spawning was
/// cancelled or to get the creep after it was spawned.
#[derive(Debug)]
pub struct SpawnPromise {
    pub id: UId,
    pub spawn_id: Option<ObjectId<StructureSpawn>>,
    pub spawn_end_tick: Option<u32>,
    pub cancelled: bool,
    pub creep: Option<CreepRef>,
}

impl SpawnPromise {
    pub fn new() -> Self {
        Self {
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