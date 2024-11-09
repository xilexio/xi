use log::trace;
use screeps::{RoomName, ENERGY_REGEN_TIME, SOURCE_ENERGY_CAPACITY};
use serde::{Deserialize, Serialize};
use crate::hauling::haul_resources::hauler_body;
use crate::kernel::sleep::sleep;
use crate::room_state::room_states::with_room_state;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HaulingStats {
    pub required_hauling_throughput: f32,
    pub required_haulers: u32,
}

impl Default for HaulingStats {
    fn default() -> Self {
        HaulingStats {
            required_hauling_throughput: 0.0,
            required_haulers: 1,
        }
    }
}

pub async fn update_hauling_stats(room_name: RoomName) {
    loop {
        trace!("Updating hauling stats.");

        with_room_state(room_name, |room_state| {
            let body = hauler_body(room_state);
            let hauler_body_capacity = body.store_capacity();
            let hauler_ticks_per_tile = body.ticks_per_tile(false);
            let hauler_throughput = hauler_body_capacity as f32 / (2 * hauler_ticks_per_tile) as f32;

            let source_energy_production = SOURCE_ENERGY_CAPACITY as f32 / ENERGY_REGEN_TIME as f32;

            let average_hauling_distance = 15.0;

            let required_hauling_throughput = source_energy_production * average_hauling_distance;
            let required_haulers = (required_hauling_throughput / hauler_throughput).ceil() as u32;
            room_state.hauling_stats = HaulingStats {
                required_hauling_throughput,
                required_haulers,
            };
            
            trace!("Updated hauling stats: {:?}", room_state.hauling_stats);
        });

        sleep(20).await;
    }
}