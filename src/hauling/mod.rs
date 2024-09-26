use screeps::{Resource, RoomName, Transferable, Withdrawable};
use crate::hauling::requests::{with_haul_requests, RawStoreRequest, RawWithdrawRequest, StoreRequest, StoreRequestId, WithdrawRequest, WithdrawRequestId};

pub mod haul_resources;
pub mod requests;

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
    with_haul_requests(store_request.room_name, |schedule| {
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

fn schedule_raw_withdraw_request(room_name: RoomName, request: crate::hauling::requests::RawWithdrawRequest, mut replaced_request_id: Option<crate::hauling::requests::WithdrawRequestId>) -> crate::hauling::requests::WithdrawRequestId {
    if let Some(mut existing_replaced_request_id) = replaced_request_id.take() {
        existing_replaced_request_id.droppable = false;
    }

    crate::hauling::requests::with_haul_requests(room_name, |schedule| {
        let id = schedule.next_id;
        schedule.next_id += 1;
        schedule.withdraw_requests.insert(id, request);
        crate::hauling::requests::WithdrawRequestId {
            room_name,
            id,
            droppable: true,
        }
    })
}