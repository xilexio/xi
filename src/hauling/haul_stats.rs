use screeps::RoomName;
use crate::utils::avg_vector::AvgVector;
use crate::hauling::requests::{with_haul_requests, HaulRequestKind, HaulRequestTargetKind};

#[derive(Debug, Default)]
pub struct HaulStats {
    /// Total amount of resources that are to be withdrawn by haulers belonging to the room.
    pub unfulfilled_withdraw_amount: AvgVector<u32>,
    /// Total amount of resources that are to be deposited by haulers belonging to the room.
    pub unfulfilled_deposit_amount: AvgVector<u32>,
    /// Total amount of resources belonging to the storages in the room.
    pub withdrawable_storage_amount: AvgVector<u32>,
    /// Total amount of free space in the storages in the room.
    pub depositable_storage_amount: AvgVector<u32>,
}

impl HaulStats {
    pub fn add_sample(&mut self, room_name: RoomName) {
        with_haul_requests(room_name, |haul_requests| {
            let mut amounts = [[0u32, 0u32], [0u32, 0u32]];
            haul_requests.withdraw_requests.values().for_each(|request| {
                let borrowed_request = request.borrow();
                let is_deposit = (borrowed_request.kind == HaulRequestKind::DepositRequest) as usize;
                let is_storage = (borrowed_request.target_kind == HaulRequestTargetKind::StorageTarget) as usize;
                amounts[is_deposit][is_storage] += borrowed_request.amount - borrowed_request.reserved_amount;
            });
            self.unfulfilled_withdraw_amount.push(amounts[0][0]);
            self.unfulfilled_deposit_amount.push(amounts[1][0]);
            self.withdrawable_storage_amount.push(amounts[0][1]);
            self.depositable_storage_amount.push(amounts[1][1]);
        });
    }
}