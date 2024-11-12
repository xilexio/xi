use rustc_hash::FxHashMap;
use screeps::{ObjectId, Position, RawObjectId, Resource, ResourceType, RoomName, Transferable, Withdrawable};
use std::cell::RefCell;
use log::debug;
use crate::utils::priority::Priority;
use crate::utils::uid::UId;

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
            .or_default();
        f(room_spawn_schedule)
    })
}

pub type RequestId = UId;

#[derive(Default)]
pub(crate) struct RoomHaulRequests {
    pub withdraw_requests: FxHashMap<RequestId, RawWithdrawRequest>,
    pub store_requests: FxHashMap<RequestId, RawStoreRequest>,
}

/// The hauler ID is the offset int the `current_haulers` vector.
type HaulerId = usize;

#[derive(Debug)]
pub struct WithdrawRequest<T> {
    /// Name of the room responsible for providing the hauler.
    pub room_name: RoomName,
    pub target: ObjectId<T>,
    pub pos: Option<Position>,
    pub resource_type: ResourceType,
    /// Amount of the resource to withdraw.
    pub amount: u32,
    // pub amount_per_tick: u32,
    // pub max_amount: u32,
    pub priority: Priority,
    // pub preferred_tick: (u32, u32),
}

#[derive(Debug)]
pub(crate) struct RawWithdrawRequest {
    /// Name of the room responsible for providing the hauler.
    pub room_name: RoomName,
    pub target: RawObjectId,
    pub pickupable: bool,
    pub pos: Option<Position>,
    pub resource_type: ResourceType,
    pub amount: u32,
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
    /// Name of the room responsible for providing the hauler.
    pub room_name: RoomName,
    pub target: ObjectId<T>,
    pub resource_type: ResourceType,
    pub pos: Option<Position>,
    /// Amount of resource to store. Can exceed the capacity in which case its maximum capacity.
    pub amount: u32,
    pub priority: Priority,
    // pub preferred_tick: (u32, u32),
}

#[derive(Debug)]
pub struct RawStoreRequest {
    /// Name of the room responsible for providing the hauler.
    pub room_name: RoomName,
    pub target: RawObjectId,
    pub pos: Option<Position>,
    pub resource_type: ResourceType,
    pub amount: u32,
    pub priority: Priority,
    // pub preferred_tick: (u32, u32),
}

/// Withdraw request identifier that cancels the request on drop.
pub struct WithdrawRequestHandle {
    room_name: RoomName,
    id: RequestId,
    droppable: bool,
}

impl Drop for WithdrawRequestHandle {
    fn drop(&mut self) {
        with_haul_requests(self.room_name, |schedule| {
            // TODO Cancelling haul that is already in progress.
            schedule.withdraw_requests.remove(&self.id);
        });
    }
}

/// Store request identifier that cancels the request on drop.
pub struct StoreRequestHandle {
    room_name: RoomName,
    id: RequestId,
    droppable: bool,
}

impl Drop for StoreRequestHandle {
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

pub fn schedule_withdraw<T>(withdraw_request: &WithdrawRequest<T>, replaced_request_id: Option<WithdrawRequestHandle>) -> WithdrawRequestHandle
    where
        T: Withdrawable,
{
    let raw_withdraw_request = RawWithdrawRequest {
        room_name: withdraw_request.room_name,
        target: withdraw_request.target.into(),
        pickupable: false,
        pos: withdraw_request.pos,
        resource_type: withdraw_request.resource_type,
        amount: withdraw_request.amount,
        // amount_per_tick: withdraw_request.amount_per_tick,
        // max_amount: withdraw_request.max_amount,
        priority: withdraw_request.priority,
        // preferred_tick: withdraw_request.preferred_tick,
        // request_tick: game_tick(),
    };
    schedule_raw_withdraw_request(withdraw_request.room_name, raw_withdraw_request, replaced_request_id)
}

pub fn schedule_pickup(withdraw_request: WithdrawRequest<Resource>, replaced_request_id: Option<WithdrawRequestHandle>) -> WithdrawRequestHandle {
    let raw_withdraw_request = RawWithdrawRequest {
        room_name: withdraw_request.room_name,
        target: withdraw_request.target.into(),
        pickupable: true,
        pos: withdraw_request.pos,
        resource_type: withdraw_request.resource_type,
        amount: withdraw_request.amount,
        // amount_per_tick: withdraw_request.amount_per_tick,
        // max_amount: withdraw_request.max_amount,
        priority: withdraw_request.priority,
        // preferred_tick: withdraw_request.preferred_tick,
        // request_tick: game_tick(),
    };
    schedule_raw_withdraw_request(withdraw_request.room_name, raw_withdraw_request, replaced_request_id)
}

fn schedule_raw_withdraw_request(room_name: RoomName, request: RawWithdrawRequest, mut replaced_request_handle: Option<WithdrawRequestHandle>) -> WithdrawRequestHandle {
    let handle = if let Some(existing_replaced_request_handle) = replaced_request_handle.take() {
        existing_replaced_request_handle
    } else {
        WithdrawRequestHandle {
            room_name,
            id: UId::new(),
            droppable: true,
        }
    };

    with_haul_requests(room_name, |schedule| {
        schedule.withdraw_requests.insert(handle.id, request);
    });

    // TODO Do something if the replaced request is already in progress.

    handle
}

pub fn schedule_store<T>(store_request: StoreRequest<T>, mut replaced_request_handle: Option<StoreRequestHandle>) -> StoreRequestHandle
where
    T: Transferable,
{
    let handle = if let Some(existing_replaced_request_handle) = replaced_request_handle.take() {
        existing_replaced_request_handle
    } else {
        let id = UId::new();
        StoreRequestHandle {
            room_name: store_request.room_name,
            id,
            droppable: true,
        }
    };

    let raw_store_request = RawStoreRequest {
        room_name: store_request.room_name,
        target: store_request.target.into(),
        pos: store_request.pos,
        resource_type: store_request.resource_type,
        amount: store_request.amount,
        priority: store_request.priority,
        // preferred_tick: store_request.preferred_tick,
    };

    with_haul_requests(store_request.room_name, |schedule| {
        schedule.store_requests.insert(handle.id, raw_store_request);
    });

    // TODO Do something if the replaced request is already in progress.

    handle
}