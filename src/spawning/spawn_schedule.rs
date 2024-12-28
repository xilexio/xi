use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use rustc_hash::FxHashMap;
use screeps::{ObjectId, RoomName, StructureSpawn};
use crate::creeps::creep_role::CreepRole;
use crate::creeps::creep_body::CreepBody;
use crate::room_states::room_state::RoomState;
use crate::spawning::preferred_spawn::{best_spawns, PreferredSpawn};
use crate::spawning::reserved_creep::ReservedCreep;
use crate::utils::priority::Priority;
use crate::utils::uid::UId;

thread_local! {
    static SPAWN_SCHEDULES: RefCell<FxHashMap<RoomName, RoomSpawnSchedule>> = RefCell::new(FxHashMap::default());
}

pub fn with_spawn_schedule<F, R>(room_name: RoomName, f: F) -> R
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

#[derive(Default, Debug)]
pub struct RoomSpawnSchedule {
    /// Future spawns ordered by preferred tick.
    pub future_spawns: BTreeMap<u32, FxHashMap<SId, SpawnEvent>>,
    /// Current spawns ordered by priority. Usually empty unless there are insufficient resources
    /// to spawn a creep.
    pub current_spawns: BTreeMap<(Priority, SId), SpawnEvent>,
    /// Spawn events for creeps currently being spawned.
    pub spawns_in_progress: FxHashMap<ObjectId<StructureSpawn>, Option<SpawnEvent>>,
}

/// A scheduled spawn.
#[derive(Debug)]
pub struct SpawnEvent {
    pub request: SpawnRequest,
    pub promise: SpawnPromiseRef,
    pub energy_cost: u32,
    pub spawn_duration: u32,
}

pub type SId = UId<'S'>;

/// A promise to spawn a creep. It can be used to check the progress, whether the spawning was
/// cancelled or to get the creep after it was spawned.
#[derive(Debug)]
pub struct SpawnPromise {
    pub id: SId,
    pub spawn_id: Option<ObjectId<StructureSpawn>>,
    pub spawn_end_tick: Option<u32>,
    pub cancelled: bool,
    pub creep: Option<ReservedCreep>,
}

impl SpawnPromise {
    pub fn new() -> Self {
        Self {
            id: SId::new(),
            spawn_id: None,
            spawn_end_tick: None,
            cancelled: false,
            creep: None,
        }
    }
    
    pub fn is_pending(&self) -> bool {
        !self.cancelled && self.creep.is_none()
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

/// A spawn request with empty body, zero tick and no spawn preference.
/// To be modified before actual spawning.
pub fn generic_base_spawn_request(room_state: &RoomState, role: CreepRole) -> SpawnRequest {
    let preferred_spawns = best_spawns(room_state, None);
    
    SpawnRequest {
        role,
        body: CreepBody::empty(),
        priority: Priority(100),
        preferred_spawns,
        tick: (0, 0),
    }
}
