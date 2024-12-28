use log::debug;
use rustc_hash::FxHashMap;
use screeps::{ResourceType, RoomName, StructureStorage, STORAGE_CAPACITY};
use screeps::game::get_object_by_id_typed;
use screeps::StructureType::Storage;
use crate::geometry::room_xy::RoomXYUtils;
use crate::hauling::requests::HaulRequest;
use crate::hauling::requests::HaulRequestKind::{DepositRequest, WithdrawRequest};
use crate::hauling::requests::HaulRequestTargetKind::StorageTarget;
use crate::hauling::scheduling_hauls::schedule_haul;
use crate::hauling::transfers::{get_free_capacity_with_object, get_used_capacities_with_object};
use crate::hauling::transfers::TransferStage::AfterAllTransfers;
use crate::kernel::wait_until_some::wait_until_some;
use crate::room_states::room_states::with_room_state;
use crate::room_states::utils::loop_until_structures_change;
use crate::u;
use crate::utils::priority::Priority;

const MAX_USED_CAPACITY: u32 = STORAGE_CAPACITY / 2;

pub async fn manage_storage(room_name: RoomName) {
    loop {
        let (storage_xy, storage_id) = wait_until_some(|| {
            with_room_state(room_name, |room_state| {
                room_state.structures_with_type::<StructureStorage>(Storage).next()
            }).flatten()
        }).await;
        
        let storage_pos = storage_xy.to_pos(room_name);
        
        let mut deposit_requests = FxHashMap::default();
        let mut withdraw_requests = FxHashMap::default();
        
        loop_until_structures_change(room_name, 1, || {
            debug!("Managing storage at {}.", storage_xy);
            
            let obj = u!(get_object_by_id_typed(&storage_id));
            let free_capacity = get_free_capacity_with_object(&obj, storage_id.into(), None, AfterAllTransfers);
            let used_capacities = get_used_capacities_with_object(&obj, storage_id.into(), AfterAllTransfers);
            
            // TODO Not only energy, but anything. But do not allow conflicts when close to full.
            
            debug!("Scheduling haul of depositable {free_capacity} energy for storage in {room_name}.");
            let previous_deposit_request = deposit_requests.remove(&ResourceType::Energy);
            // The previous deposit request is replaced by this one.
            let mut deposit_request = HaulRequest::new(
                DepositRequest,
                room_name,
                ResourceType::Energy,
                storage_id,
                StorageTarget,
                false,
                storage_pos
            );
            deposit_request.amount = free_capacity;
            deposit_request.priority = Priority(100);
            deposit_requests.insert(
                ResourceType::Energy,
                schedule_haul(deposit_request, previous_deposit_request)
            );
            
            let previous_withdraw_request = withdraw_requests.remove(&ResourceType::Energy);
            if let Some(&used_capacity) = used_capacities.get(&ResourceType::Energy) {
                debug!("Scheduling haul of withdrawable {used_capacity} energy for storage in {room_name}.");
                // The previous withdraw request is replaced by this one.
                let mut withdraw_request = HaulRequest::new(
                    WithdrawRequest,
                    room_name,
                    ResourceType::Energy,
                    storage_id,
                    StorageTarget,
                    false,
                    storage_pos
                );
                withdraw_request.amount = used_capacity;
                withdraw_request.priority = Priority(100);
                withdraw_requests.insert(
                    ResourceType::Energy,
                    schedule_haul(withdraw_request, previous_withdraw_request)
                );
            }
            
            true
        }).await;
    }
}