use screeps::RoomName;
use crate::algorithms::avg_vector::AvgVector;
use crate::hauling::requests::{with_haul_requests, HaulRequestKind, HaulRequestTargetKind};

const HAUL_STATS_AVG_SAMPLES: usize = 100;
const HAUL_STATS_AVG_SMALL_SAMPLES: usize = 10;

pub type HaulStatsAvgVector = AvgVector<u32, HAUL_STATS_AVG_SAMPLES, HAUL_STATS_AVG_SMALL_SAMPLES>;

#[derive(Debug, Default)]
pub struct HaulStats {
    /// Total amount of resources that are to be withdrawn by haulers belonging to the room.
    pub unfulfilled_withdraw_amount: HaulStatsAvgVector,
    /// Total amount of resources that are to be deposited by haulers belonging to the room.
    pub unfulfilled_deposit_amount: HaulStatsAvgVector,
    /// Total amount of resources belonging to the storages in the room.
    pub withdrawable_storage_amount: HaulStatsAvgVector,
    /// Total amount of free space in the storages in the room.
    pub depositable_storage_amount: HaulStatsAvgVector,
    /// Number of haulers that are idle in the current tick.
    pub idle_haulers: HaulStatsAvgVector,
}

impl HaulStats {
    pub fn add_sample(&mut self, room_name: RoomName, idle_haulers: u32) {
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
            self.idle_haulers.push(idle_haulers);
        });
    }
}