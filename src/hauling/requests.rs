use crate::kernel::process::Priority;
use rustc_hash::FxHashMap;
use screeps::{ObjectId, Position, RawObjectId, ResourceType, RoomName, Transferable};
use std::cell::RefCell;
use log::debug;

thread_local! {
    static HAUL_REQUESTS: RefCell<FxHashMap<RoomName, RoomHaulRequests >> = RefCell::new(FxHashMap::default());
}

pub(super) fn with_haul_requests<F, R>(room_name: RoomName, f: F) -> R
where
    F: FnOnce(&mut RoomHaulRequests) -> R,
{
    // TODO need scan data to create the schedule
    HAUL_REQUESTS.with(|states| {
        let mut borrowed_states = states.borrow_mut();
        let room_spawn_schedule = borrowed_states
            .entry(room_name)
            .or_insert_with(RoomHaulRequests::default);
        f(room_spawn_schedule)
    })
}

pub type RequestId = u32;

#[derive(Default)]
pub(super) struct RoomHaulRequests {
    pub withdraw_requests: FxHashMap<RequestId, RawWithdrawRequest>,
    pub store_requests: FxHashMap<RequestId, RawStoreRequest>,
    pub next_id: u32,
}

/// The hauler ID is the offset int the `current_haulers` vector.
type HaulerId = usize;

#[derive(Debug)]
pub struct WithdrawRequest<T> {
    /// Room name of the room responsible for providing the hauler.
    pub room_name: RoomName,
    pub target: ObjectId<T>,
    pub xy: Option<Position>,
    pub resource_type: ResourceType,
    /// Amount of the resource to withdraw or all that is possible.
    pub amount: Option<u32>,
    // pub amount_per_tick: u32,
    // pub max_amount: u32,
    pub priority: Priority,
    // pub preferred_tick: (u32, u32),
}

#[derive(Debug)]
pub(super) struct RawWithdrawRequest {
    pub target: RawObjectId,
    pub pickupable: bool,
    pub xy: Option<Position>,
    pub resource_type: ResourceType,
    pub amount: Option<u32>,
    // amount_per_tick: u32,
    // max_amount: u32,
    pub priority: Priority,
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
    pub resource_type: ResourceType,
    pub xy: Option<Position>,
    /// Amount of resource to store or all that is possible.
    pub amount: Option<u32>,
    pub priority: Priority,
    // pub preferred_tick: (u32, u32),
}

#[derive(Debug)]
pub struct RawStoreRequest {
    pub target: RawObjectId,
    pub xy: Option<Position>,
    pub resource_type: ResourceType,
    pub amount: Option<u32>,
    pub priority: Priority,
    // pub preferred_tick: (u32, u32),
}

// TODO Option to not drop it.
pub struct WithdrawRequestId {
    pub room_name: RoomName,
    pub id: RequestId,
    pub droppable: bool,
}

impl Drop for WithdrawRequestId {
    fn drop(&mut self) {
        with_haul_requests(self.room_name, |schedule| {
            // TODO Cancelling haul that is already in progress.
            schedule.withdraw_requests.remove(&self.id);
        });
    }
}

pub struct StoreRequestId {
    pub room_name: RoomName,
    pub id: RequestId,
    pub droppable: bool,
}

impl Drop for StoreRequestId {
    fn drop(&mut self) {
        with_haul_requests(self.room_name, |schedule| {
            // TODO Cancelling haul that is already in progress.
            if self.droppable {
                debug!("Dropping store request {}/{}.", self.room_name, self.id);
                schedule.store_requests.remove(&self.id);
            }
        });
    }
}